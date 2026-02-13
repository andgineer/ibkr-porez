from __future__ import annotations

from datetime import date, datetime

import allure
import pytest

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


@allure.epic("GUI")
@allure.feature("Main Window")
class TestGuiMainWindowUnit:
    def test_visible_declaration_indices_respects_active_all_and_status_filters(self) -> None:
        window = MainWindow.__new__(MainWindow)
        window.declarations = [
            _declaration("d1", DeclarationStatus.DRAFT),
            _declaration("d2", DeclarationStatus.SUBMITTED),
            _declaration("d3", DeclarationStatus.FINALIZED),
            _declaration("d4", DeclarationStatus.PENDING),
        ]

        window.status_filter = "Active"
        assert window._visible_declaration_indices() == [0, 1, 3]

        window.status_filter = "All"
        assert window._visible_declaration_indices() == [0, 1, 2, 3]

        window.status_filter = "Pending payment"
        assert window._visible_declaration_indices() == [1, 3]

    @pytest.mark.parametrize(
        ("declaration_status", "target_status", "expected_method"),
        [
            (DeclarationStatus.DRAFT, "Submitted", "submit"),
            (DeclarationStatus.DRAFT, "Finalized", "pay"),
            (DeclarationStatus.PENDING, "Finalized", "pay"),
            (DeclarationStatus.SUBMITTED, "Draft", "revert"),
        ],
    )
    def test_apply_status_to_ids_dispatches_to_declaration_manager(
        self,
        declaration_status: DeclarationStatus,
        target_status: str,
        expected_method: str,
    ) -> None:
        called: list[tuple[str, list[str]]] = []

        class FakeManager:
            @staticmethod
            def has_tax_to_pay(_declaration: Declaration) -> bool:
                return True

            @staticmethod
            def has_assessed_tax(_declaration: Declaration) -> bool:
                return True

            def submit(self, ids: list[str]) -> None:
                called.append(("submit", ids))

            def pay(self, ids: list[str]) -> None:
                called.append(("pay", ids))

            def revert(self, ids: list[str], _target: DeclarationStatus) -> None:
                called.append(("revert", ids))

        window = MainWindow.__new__(MainWindow)
        window.declarations = [_declaration("d1", declaration_status)]
        window.declaration_manager = FakeManager()

        window.apply_status_to_ids(["d1"], target_status)

        assert called == [(expected_method, ["d1"])]

    def test_apply_status_to_ids_raises_for_missing_declaration_id(self) -> None:
        class FakeManager:
            def submit(self, ids: list[str]) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

            def pay(self, ids: list[str]) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

            def revert(self, ids: list[str], _target: DeclarationStatus) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

        window = MainWindow.__new__(MainWindow)
        window.declarations = [_declaration("known", DeclarationStatus.DRAFT)]
        window.declaration_manager = FakeManager()

        with pytest.raises(ValueError, match="unknown"):
            window.apply_status_to_ids(["unknown"], "Submitted")

    def test_apply_status_to_ids_raises_for_invalid_transition(self) -> None:
        class FakeManager:
            def submit(self, ids: list[str]) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

            def pay(self, ids: list[str]) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

            def revert(self, ids: list[str], _target: DeclarationStatus) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

        window = MainWindow.__new__(MainWindow)
        window.declarations = [_declaration("finalized-decl", DeclarationStatus.FINALIZED)]
        window.declaration_manager = FakeManager()

        with pytest.raises(ValueError, match="finalized-decl"):
            window.apply_status_to_ids(["finalized-decl"], "Submitted")

    def test_apply_status_to_ids_disallows_finalized_when_no_tax_due(self) -> None:
        class FakeManager:
            @staticmethod
            def has_tax_to_pay(_declaration: Declaration) -> bool:
                return False

            def submit(self, ids: list[str]) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

            def pay(self, ids: list[str]) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

            def revert(self, ids: list[str], _target: DeclarationStatus) -> None:  # noqa: ARG002
                raise AssertionError("should not be called")

        window = MainWindow.__new__(MainWindow)
        window.declarations = [_declaration("zero-tax", DeclarationStatus.DRAFT)]
        window.declaration_manager = FakeManager()

        with pytest.raises(ValueError, match="zero-tax"):
            window.apply_status_to_ids(["zero-tax"], "Finalized")

    def test_open_declaration_details_by_view_row_opens_dialog_for_selected_row(
        self,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        opened: list[str] = []

        class FakeDialog:
            def __init__(self, declaration_id: str, _parent: MainWindow) -> None:
                opened.append(declaration_id)

            def exec(self) -> int:
                opened.append("exec")
                return 0

        monkeypatch.setattr("ibkr_porez.gui.main_window.DeclarationDetailsDialog", FakeDialog)

        window = MainWindow.__new__(MainWindow)
        window.declarations = [
            _declaration("d0", DeclarationStatus.DRAFT),
            _declaration("d1", DeclarationStatus.SUBMITTED),
        ]
        window.visible_indices = [1]

        window.open_declaration_details_by_view_row(0)

        assert opened == ["d1", "exec"]

    def test_open_declaration_details_by_view_row_ignores_invalid_row(
        self,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        opened: list[str] = []

        class FakeDialog:
            def __init__(self, declaration_id: str, _parent: MainWindow) -> None:
                opened.append(declaration_id)

            def exec(self) -> int:
                opened.append("exec")
                return 0

        monkeypatch.setattr("ibkr_porez.gui.main_window.DeclarationDetailsDialog", FakeDialog)

        window = MainWindow.__new__(MainWindow)
        window.declarations = [_declaration("d0", DeclarationStatus.DRAFT)]
        window.visible_indices = [0]

        window.open_declaration_details_by_view_row(99)

        assert opened == []
