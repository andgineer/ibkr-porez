from __future__ import annotations

import importlib.util
from datetime import date, datetime

import allure
import pytest
from PySide6.QtCore import Qt

import ibkr_porez.gui.main_window as main_window_module
from ibkr_porez.declaration_manager import DeclarationManager as RealDeclarationManager
from ibkr_porez.gui.main_window import MainWindow
from ibkr_porez.models import Declaration, DeclarationStatus, DeclarationType, UserConfig

HAS_PYTEST_QT = importlib.util.find_spec("pytestqt") is not None


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

    def __init__(self) -> None:
        return


@pytest.fixture
def qtbot_or_skip(request):
    if not HAS_PYTEST_QT:
        pytest.skip("pytest-qt is not installed")
    return request.getfixturevalue("qtbot")


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
            declaration_id="2026-01-ppdg-paid",
            type=DeclarationType.PPDG3R,
            status=DeclarationStatus.PAID,
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
def test_qtbot_renders_main_window(qtbot_or_skip, patched_main_window: MainWindow) -> None:
    qtbot_or_skip.addWidget(patched_main_window)
    patched_main_window.show()
    qtbot_or_skip.waitUntil(lambda: patched_main_window.table.rowCount() == 2)

    assert patched_main_window.windowTitle() == "ibkr-porez"
    assert patched_main_window.sync_button.text().endswith("Sync")
    assert patched_main_window.filter_combo.currentText() == "Active"
    assert patched_main_window.table.item(0, 0).text() == "2026-02-03-ppo-aapl"


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_can_change_filter(qtbot_or_skip, patched_main_window: MainWindow) -> None:
    qtbot_or_skip.addWidget(patched_main_window)
    patched_main_window.show()
    patched_main_window.filter_combo.setCurrentText("Draft")
    qtbot_or_skip.waitUntil(lambda: patched_main_window.table.rowCount() == 1)

    assert patched_main_window.table.item(0, 0).text() == "2026-q1-ppdg"


@allure.epic("GUI")
@allure.feature("pytest-qt")
def test_qtbot_selection_shows_bulk_controls(
    qtbot_or_skip,
    patched_main_window: MainWindow,
) -> None:
    qtbot_or_skip.addWidget(patched_main_window)
    patched_main_window.show()

    assert not patched_main_window.bulk_status_combo.isVisible()
    assert not patched_main_window.apply_status_button.isVisible()

    qtbot_or_skip.mouseClick(patched_main_window.select_all_button, Qt.MouseButton.LeftButton)
    qtbot_or_skip.waitUntil(lambda: patched_main_window.selection_info_label.text() == "2 selected")

    assert patched_main_window.bulk_status_combo.isVisible()
    assert patched_main_window.apply_status_button.isVisible()
    assert patched_main_window.apply_status_button.isEnabled()

    qtbot_or_skip.mouseClick(
        patched_main_window.clear_selection_button,
        Qt.MouseButton.LeftButton,
    )
    qtbot_or_skip.waitUntil(lambda: patched_main_window.selection_info_label.text() == "0 selected")

    assert not patched_main_window.bulk_status_combo.isVisible()
    assert not patched_main_window.apply_status_button.isVisible()
