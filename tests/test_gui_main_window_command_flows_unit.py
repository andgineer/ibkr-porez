from __future__ import annotations

from dataclasses import dataclass
from datetime import date, datetime
from decimal import Decimal

import allure
import pytest
from PySide6.QtWidgets import QDialog

import ibkr_porez.gui.main_window as main_window_module
from ibkr_porez.gui.constants import PROGRESS_MAX
from ibkr_porez.gui.main_window import MainWindow
from ibkr_porez.models import Declaration, DeclarationStatus, DeclarationType


def _declaration(declaration_id: str, status: DeclarationStatus) -> Declaration:
    return Declaration(
        declaration_id=declaration_id,
        type=DeclarationType.PPDG3R,
        status=status,
        period_start=date(2026, 1, 1),
        period_end=date(2026, 1, 31),
        created_at=datetime(2026, 2, 1, 10, 0, 0),
    )


@dataclass
class _FakeProgressBar:
    minimum: int = 0
    maximum: int = PROGRESS_MAX
    value: int = 0
    visible: bool = False

    def setRange(self, minimum: int, maximum: int) -> None:  # noqa: N802
        self.minimum = minimum
        self.maximum = maximum

    def setValue(self, value: int) -> None:  # noqa: N802
        self.value = value

    def setVisible(self, visible: bool) -> None:  # noqa: N802
        self.visible = visible


@dataclass
class _FakeButton:
    enabled: bool = True

    def setEnabled(self, enabled: bool) -> None:  # noqa: N802
        self.enabled = enabled


@dataclass
class _FakeLabel:
    value: str = ""

    def setText(self, text: str) -> None:  # noqa: N802
        self.value = text

    def text(self) -> str:
        return self.value


@dataclass
class _FakeControl:
    visible: bool = False
    enabled: bool = True

    def setVisible(self, visible: bool) -> None:  # noqa: N802
        self.visible = visible

    def setEnabled(self, enabled: bool) -> None:  # noqa: N802
        self.enabled = enabled


@allure.epic("GUI")
@allure.feature("Main Window Commands")
class TestGuiMainWindowCommandFlowsUnit:
    def test_assess_success_updates_data_and_shows_summary(
        self,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        declaration = _declaration("d1", DeclarationStatus.SUBMITTED)

        class FakeAssessmentDialog:
            tax_due_rsd = Decimal("123.45")
            mark_paid = False

            def __init__(self, declaration_id: str, initial_tax_due_rsd: Decimal | None, parent):
                assert declaration_id == "d1"
                assert initial_tax_due_rsd is None
                assert parent is window

            @staticmethod
            def exec() -> int:
                return int(QDialog.DialogCode.Accepted)

        class FakeManager:
            @staticmethod
            def set_assessed_tax(
                declaration_id: str,
                tax_due_rsd: Decimal,
                mark_paid: bool,
            ) -> Declaration:
                assert declaration_id == "d1"
                assert tax_due_rsd == Decimal("123.45")
                assert mark_paid is False
                return declaration.model_copy(update={"status": DeclarationStatus.SUBMITTED})

            @staticmethod
            def assessment_message(
                declaration_id: str,
                tax_due_rsd: Decimal,
                status: DeclarationStatus,
                mark_paid: bool,
            ) -> str:
                assert declaration_id == "d1"
                assert tax_due_rsd == Decimal("123.45")
                assert status == DeclarationStatus.SUBMITTED
                assert mark_paid is False
                return "assessment-ok"

        monkeypatch.setattr(main_window_module, "AssessmentDialog", FakeAssessmentDialog)

        calls: dict[str, object] = {}
        window = MainWindow.__new__(MainWindow)
        window.declaration_manager = FakeManager()
        window.declarations = [declaration]
        window.reload_declarations = lambda: calls.__setitem__("reloaded", True)
        window.populate_table = lambda reselect_declaration_ids=None: calls.__setitem__(
            "reselected",
            reselect_declaration_ids,
        )
        window._show_command_success = lambda message: calls.__setitem__("success", message)
        window._show_command_error = lambda *args: calls.__setitem__("error", args)

        window.open_assessment_dialog(0)

        assert calls["reloaded"] is True
        assert calls["reselected"] == ["d1"]
        assert calls["success"] == "assessment-ok"
        assert "error" not in calls

    def test_assess_failure_shows_error_dialog_summary(
        self,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        declaration = _declaration("d1", DeclarationStatus.SUBMITTED)

        class FakeAssessmentDialog:
            tax_due_rsd = Decimal("10.00")
            mark_paid = True

            def __init__(self, *_args, **_kwargs):
                return

            @staticmethod
            def exec() -> int:
                return int(QDialog.DialogCode.Accepted)

        class FakeManager:
            @staticmethod
            def set_assessed_tax(*_args, **_kwargs) -> Declaration:
                raise ValueError("assessment failed")

        monkeypatch.setattr(main_window_module, "AssessmentDialog", FakeAssessmentDialog)

        calls: dict[str, object] = {}
        window = MainWindow.__new__(MainWindow)
        window.declaration_manager = FakeManager()
        window.declarations = [declaration]
        window.reload_declarations = lambda: calls.__setitem__("reloaded", True)
        window.populate_table = lambda **_kwargs: calls.__setitem__("populated", True)
        window._show_command_success = lambda message: calls.__setitem__("success", message)
        window._show_command_error = lambda title, message, summary: calls.__setitem__(
            "error",
            (title, message, summary),
        )

        window.open_assessment_dialog(0)

        assert calls.get("error") == (
            "Assessment error",
            "assessment failed",
            "Assessment update failed",
        )
        assert "reloaded" not in calls
        assert "success" not in calls

    def test_sync_success_updates_progress_and_message(self) -> None:
        calls: dict[str, object] = {}
        window = MainWindow.__new__(MainWindow)
        window.progress_bar = _FakeProgressBar()
        window.reload_declarations = lambda: calls.__setitem__("reloaded", True)
        window.populate_table = lambda: calls.__setitem__("populated", True)
        window._show_command_success = lambda message: calls.__setitem__("success", message)

        window.on_sync_finished(2, "/tmp/output")

        assert calls["reloaded"] is True
        assert calls["populated"] is True
        assert calls["success"] == "Sync complete, created 2 declaration(s) in /tmp/output"
        assert window.progress_bar.minimum == 0
        assert window.progress_bar.maximum == PROGRESS_MAX
        assert window.progress_bar.value == PROGRESS_MAX

    def test_sync_failure_updates_progress_and_error_summary(self) -> None:
        calls: dict[str, object] = {}
        window = MainWindow.__new__(MainWindow)
        window.progress_bar = _FakeProgressBar(minimum=0, maximum=0, value=5, visible=True)
        window._show_command_error = lambda title, message, summary: calls.__setitem__(
            "error",
            (title, message, summary),
        )

        window.on_sync_failed("network timeout")

        assert calls["error"] == ("Sync error", "network timeout", "Sync failed")
        assert window.progress_bar.minimum == 0
        assert window.progress_bar.maximum == PROGRESS_MAX
        assert window.progress_bar.value == 0

    def test_sync_thread_finished_hides_progress_and_reenables_sync_button(self) -> None:
        window = MainWindow.__new__(MainWindow)
        window.progress_bar = _FakeProgressBar(visible=True)
        window.sync_button = _FakeButton(enabled=False)
        window.sync_worker = object()
        window.sync_thread = object()

        window.on_sync_thread_finished()

        assert window.progress_bar.visible is False
        assert window.sync_button.enabled is True
        assert window.sync_worker is None
        assert window.sync_thread is None

    def test_export_success_and_failure_update_summary(self) -> None:
        calls: dict[str, object] = {}
        window = MainWindow.__new__(MainWindow)
        window._finish_export_ui_state = lambda: calls.__setitem__(
            "finish_calls",
            calls.get("finish_calls", 0) + 1,
        )
        window._show_command_success = lambda message: calls.__setitem__("success", message)
        window._show_command_error = lambda title, message, summary: calls.__setitem__(
            "error",
            (title, message, summary),
        )

        window.on_export_finished("/tmp/exported")
        window.on_export_failed("disk full")

        assert calls["success"] == "Declaration files saved in /tmp/exported"
        assert calls["error"] == ("Export error", "disk full", "Re-export failed")
        assert calls["finish_calls"] == 2

    def test_start_sync_with_empty_history_cancelled_shows_warning_message(self) -> None:
        calls: dict[str, object] = {}
        window = MainWindow.__new__(MainWindow)
        window.sync_thread = None
        window._is_transactions_history_empty = lambda: True
        window._confirm_action = lambda **_kwargs: False
        window._update_empty_transactions_warning = lambda force=False: calls.__setitem__(
            "warning_force", force
        )

        window._start_sync(forced=False)

        assert calls["warning_force"] is True

    def test_start_forced_sync_confirmation_gates_execution(self) -> None:
        calls: dict[str, object] = {}
        window = MainWindow.__new__(MainWindow)
        window._start_sync = lambda forced: calls.__setitem__("forced", forced)

        window._confirm_action = lambda **_kwargs: False
        window.start_forced_sync()
        assert "forced" not in calls

        window._confirm_action = lambda **_kwargs: True
        window.start_forced_sync()
        assert calls["forced"] is True

    def test_update_selection_info_toggles_bulk_controls_by_selection_count(self) -> None:
        window = MainWindow.__new__(MainWindow)
        window.selection_info_label = _FakeLabel()
        window.bulk_status_combo = _FakeControl()
        window.apply_status_button = _FakeControl()

        window.selected_source_rows = lambda: []
        window.update_selection_info()
        assert window.selection_info_label.text() == "0 selected"
        assert window.bulk_status_combo.visible is False
        assert window.apply_status_button.visible is False
        assert window.apply_status_button.enabled is False

        window.selected_source_rows = lambda: [1, 2]
        window.update_selection_info()
        assert window.selection_info_label.text() == "2 selected"
        assert window.bulk_status_combo.visible is True
        assert window.apply_status_button.visible is True
        assert window.apply_status_button.enabled is True
