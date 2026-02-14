from __future__ import annotations

import importlib.util
from datetime import date, datetime
from pathlib import Path

import allure
import pytest
from PySide6.QtWidgets import QApplication, QToolButton

import ibkr_porez.gui.main_window as main_window_module
from ibkr_porez.declaration_manager import DeclarationManager as RealDeclarationManager
from ibkr_porez.gui.constants import FILTER_ORDER
from ibkr_porez.gui.main_window import MainWindow
from ibkr_porez.models import Declaration, DeclarationStatus, DeclarationType, UserConfig

BASELINE_DIR = Path(__file__).parent / "resources" / "regressions"
HAS_PYTEST_REGRESSIONS = importlib.util.find_spec("pytest_regressions") is not None


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


@pytest.fixture(scope="module")
def qapp() -> QApplication:
    app = QApplication.instance()
    if app is None:
        app = QApplication(["pytest"])
    return app


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


def _row_action_snapshot(window: MainWindow, row: int) -> list[dict[str, str | bool]]:
    cell_widget = window.table.cellWidget(row, 6)
    if cell_widget is None or cell_widget.layout() is None:
        return []
    layout = cell_widget.layout()
    actions: list[dict[str, str | bool]] = []
    for index in range(layout.count()):
        widget = layout.itemAt(index).widget()
        if isinstance(widget, QToolButton):
            actions.append(
                {
                    "text": widget.text(),
                    "enabled": widget.isEnabled(),
                },
            )
    return actions


def _normalize_button_text(text: str) -> str:
    return text.replace("↻", "").strip()


def _table_rows_snapshot(window: MainWindow) -> list[dict[str, str | list[dict[str, str | bool]]]]:
    rows = []
    for row in range(window.table.rowCount()):
        rows.append(
            {
                "id": window.table.item(row, 0).text(),
                "type": window.table.item(row, 1).text(),
                "period": window.table.item(row, 2).text(),
                "tax": window.table.item(row, 3).text(),
                "status": window.table.item(row, 4).text(),
                "created": window.table.item(row, 5).text(),
                "actions": _row_action_snapshot(window, row),
            },
        )
    return rows


@allure.epic("GUI")
@allure.feature("Regression")
@pytest.mark.skipif(
    not HAS_PYTEST_REGRESSIONS,
    reason="pytest-regressions is not installed",
)
def test_main_window_active_layout_regression(
    qapp: QApplication,  # noqa: ARG001
    patched_main_window: MainWindow,
    data_regression,
) -> None:
    snapshot = {
        "window_title": patched_main_window.windowTitle(),
        "top_buttons": [
            _normalize_button_text(patched_main_window.sync_button.text()),
            _normalize_button_text(patched_main_window.import_button.text()),
            _normalize_button_text(patched_main_window.config_button.text()),
        ],
        "filter_items": [
            patched_main_window.filter_combo.itemText(i)
            for i in range(patched_main_window.filter_combo.count())
        ],
        "active_filter": patched_main_window.filter_combo.currentText(),
        "table_headers": [
            patched_main_window.table.horizontalHeaderItem(i).text()
            for i in range(patched_main_window.table.columnCount())
        ],
        "rows": _table_rows_snapshot(patched_main_window),
        "selection_label": patched_main_window.selection_info_label.text(),
        "bulk_controls_visible": {
            "status_combo": patched_main_window.bulk_status_combo.isVisible(),
            "apply_button": patched_main_window.apply_status_button.isVisible(),
        },
        "progress_label": patched_main_window.progress_label.text(),
        "progress_bar_visible": patched_main_window.progress_bar.isVisible(),
    }
    data_regression.check(
        snapshot,
        fullpath=BASELINE_DIR / "gui_main_window_active.yml",
    )


@allure.epic("GUI")
@allure.feature("Regression")
@pytest.mark.skipif(
    not HAS_PYTEST_REGRESSIONS,
    reason="pytest-regressions is not installed",
)
def test_main_window_filtering_regression(
    qapp: QApplication,  # noqa: ARG001
    patched_main_window: MainWindow,
    data_regression,
) -> None:
    filter_snapshot: dict[str, dict[str, int | list[str]]] = {}
    for filter_name in FILTER_ORDER:
        patched_main_window.set_status_filter(filter_name)
        filter_snapshot[filter_name] = {
            "row_count": patched_main_window.table.rowCount(),
            "ids": [
                patched_main_window.table.item(row, 0).text()
                for row in range(patched_main_window.table.rowCount())
            ],
        }

    data_regression.check(
        filter_snapshot,
        fullpath=BASELINE_DIR / "gui_main_window_filters.yml",
    )
