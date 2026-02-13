from __future__ import annotations

import os
import sys
from datetime import date, datetime

import allure
import pytest
from PySide6.QtCore import Qt
from PySide6.QtWidgets import QToolButton

import ibkr_porez.gui.main_window as main_window_module
from ibkr_porez.declaration_manager import DeclarationManager as RealDeclarationManager
from ibkr_porez.gui.main_window import MainWindow
from ibkr_porez.models import Declaration, DeclarationStatus, DeclarationType, UserConfig

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
pytestmark = pytest.mark.skipif(
    sys.platform != "linux",
    reason="Qt UI tests run in CI only on Linux",
)


class _FakeStorage:
    def __init__(self, declarations: list[Declaration]) -> None:
        self._declarations = [declaration.model_copy(deep=True) for declaration in declarations]

    def get_declarations(self) -> list[Declaration]:
        return [declaration.model_copy(deep=True) for declaration in self._declarations]

    @staticmethod
    def get_last_transaction_date() -> date:
        return date(2026, 2, 1)


class _FakeDeclarationManager:
    is_transition_allowed = staticmethod(RealDeclarationManager.is_transition_allowed)
    has_tax_to_pay = staticmethod(lambda _declaration: True)

    def __init__(self) -> None:
        return


@pytest.fixture
def sample_declarations() -> list[Declaration]:
    return [
        Declaration(
            declaration_id="2026-02-03-ppo-aapl",
            type=DeclarationType.PPO,
            status=DeclarationStatus.SUBMITTED,
            period_start=date(2026, 1, 15),
            period_end=date(2026, 1, 15),
            created_at=datetime(2026, 2, 3, 11, 30, 0),
            metadata={"symbol": "AAPL"},
        ),
        Declaration(
            declaration_id="2026-q1-ppdg",
            type=DeclarationType.PPDG3R,
            status=DeclarationStatus.DRAFT,
            period_start=date(2026, 1, 1),
            period_end=date(2026, 1, 31),
            created_at=datetime(2026, 2, 2, 8, 15, 0),
        ),
        Declaration(
            declaration_id="2026-01-ppdg-finalized",
            type=DeclarationType.PPDG3R,
            status=DeclarationStatus.FINALIZED,
            period_start=date(2026, 1, 10),
            period_end=date(2026, 1, 10),
            created_at=datetime(2026, 2, 1, 9, 0, 0),
        ),
    ]


@pytest.fixture
def patched_main_window(monkeypatch, sample_declarations: list[Declaration]) -> MainWindow:
    monkeypatch.setattr(
        main_window_module.config_manager,
        "load_config",
        lambda: UserConfig(full_name="GUI Test User", address="GUI Test Address"),
    )
    monkeypatch.setattr(
        main_window_module,
        "Storage",
        lambda: _FakeStorage(sample_declarations),
    )
    monkeypatch.setattr(
        main_window_module,
        "DeclarationManager",
        _FakeDeclarationManager,
    )
    window = MainWindow()
    try:
        yield window
    finally:
        window.close()


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_renders_main_window(qtbot, patched_main_window: MainWindow) -> None:
    qtbot.addWidget(patched_main_window)
    patched_main_window.show()
    qtbot.waitUntil(lambda: patched_main_window.table.rowCount() == 2)

    assert patched_main_window.windowTitle() == "ibkr-porez"
    assert patched_main_window.sync_button.text().endswith("Sync")
    assert isinstance(patched_main_window.sync_button, QToolButton)
    assert patched_main_window.filter_combo.currentText() == "Active"
    assert patched_main_window.table.item(0, 0).text() == "2026-02-03-ppo-aapl"


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_can_change_filter(qtbot, patched_main_window: MainWindow) -> None:
    qtbot.addWidget(patched_main_window)
    patched_main_window.show()
    patched_main_window.filter_combo.setCurrentText("Pending payment")
    qtbot.waitUntil(lambda: patched_main_window.table.rowCount() == 1)

    assert patched_main_window.table.item(0, 0).text() == "2026-02-03-ppo-aapl"


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_selection_shows_bulk_controls(
    qtbot,
    patched_main_window: MainWindow,
) -> None:
    qtbot.addWidget(patched_main_window)
    patched_main_window.show()

    assert not patched_main_window.bulk_status_combo.isVisible()
    assert not patched_main_window.apply_status_button.isVisible()

    qtbot.mouseClick(patched_main_window.select_all_button, Qt.MouseButton.LeftButton)
    qtbot.waitUntil(lambda: patched_main_window.selection_info_label.text() == "2 selected")

    assert patched_main_window.bulk_status_combo.isVisible()
    assert patched_main_window.apply_status_button.isVisible()
    assert patched_main_window.apply_status_button.isEnabled()

    qtbot.mouseClick(
        patched_main_window.clear_selection_button,
        Qt.MouseButton.LeftButton,
    )
    qtbot.waitUntil(lambda: patched_main_window.selection_info_label.text() == "0 selected")

    assert not patched_main_window.bulk_status_combo.isVisible()
    assert not patched_main_window.apply_status_button.isVisible()


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_sync_button_has_force_action(qtbot, patched_main_window: MainWindow) -> None:
    qtbot.addWidget(patched_main_window)
    patched_main_window.show()
    menu = patched_main_window.sync_button.menu()
    assert menu is not None
    assert [action.text() for action in menu.actions()] == ["Force sync"]
    assert (
        menu.actions()[0].toolTip()
        == "Ignore last sync date, rescan recent history, and create declarations even if "
        "withholding tax is not found."
    )


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_finalized_row_has_revert_action(qtbot, patched_main_window: MainWindow) -> None:
    qtbot.addWidget(patched_main_window)
    patched_main_window.show()
    patched_main_window.filter_combo.setCurrentText("All")
    qtbot.waitUntil(lambda: patched_main_window.table.rowCount() == 3)

    finalized_row = None
    for row in range(patched_main_window.table.rowCount()):
        if patched_main_window.table.item(row, 0).text() == "2026-01-ppdg-finalized":
            finalized_row = row
            break

    assert finalized_row is not None
    row_widget = patched_main_window.table.cellWidget(finalized_row, 6)
    assert row_widget is not None
    buttons = row_widget.findChildren(QToolButton)
    button_by_text = {button.text(): button for button in buttons}
    assert "Submit" not in button_by_text
    assert "Pay" not in button_by_text
    assert "Revert" in button_by_text
    assert "Set tax" in button_by_text
    assert button_by_text["Revert"].isEnabled()


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_double_click_opens_declaration_details(
    qtbot,
    patched_main_window: MainWindow,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    opened: list[str] = []

    class FakeDialog:
        def __init__(self, declaration_id: str, _parent: MainWindow) -> None:
            opened.append(declaration_id)

        def exec(self) -> int:
            opened.append("exec")
            return 0

    monkeypatch.setattr(main_window_module, "DeclarationDetailsDialog", FakeDialog)

    qtbot.addWidget(patched_main_window)
    patched_main_window.show()
    qtbot.waitUntil(lambda: patched_main_window.table.rowCount() > 0)

    first_item = patched_main_window.table.item(0, 0)
    assert first_item is not None
    patched_main_window.table.itemDoubleClicked.emit(first_item)

    assert opened == ["2026-02-03-ppo-aapl", "exec"]


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_enter_opens_declaration_details(
    qtbot,
    patched_main_window: MainWindow,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    opened: list[str] = []

    class FakeDialog:
        def __init__(self, declaration_id: str, _parent: MainWindow) -> None:
            opened.append(declaration_id)

        def exec(self) -> int:
            opened.append("exec")
            return 0

    monkeypatch.setattr(main_window_module, "DeclarationDetailsDialog", FakeDialog)

    qtbot.addWidget(patched_main_window)
    patched_main_window.show()
    qtbot.waitUntil(lambda: patched_main_window.table.rowCount() > 0)
    patched_main_window.table.setCurrentCell(0, 0)
    patched_main_window.table.selectRow(0)
    patched_main_window.open_details_shortcut_return.activated.emit()

    assert opened == ["2026-02-03-ppo-aapl", "exec"]
