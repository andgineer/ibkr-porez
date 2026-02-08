from __future__ import annotations

import json
import time
from dataclasses import asdict, dataclass
from pathlib import Path

from PySide6.QtCore import QItemSelectionModel, QObject, Qt, QThread, Signal, Slot
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QButtonGroup,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QSpinBox,
    QTableWidget,
    QTableWidgetItem,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

CONFIG_PATH = Path.home() / ".ibkr-porez-mvp-config.json"
STATUSES = ("Active", "Submitted", "Draft")
PROGRESS_MAX = 100
PROGRESS_STEP = 12
PROGRESS_FAIL_AT = 60


@dataclass
class Config:
    account_id: str = "U1234567"
    api_token: str = ""
    sync_interval_min: int = 30


class ConfigDialog(QDialog):
    def __init__(self, config: Config, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Config")
        self.setModal(True)

        form = QFormLayout()

        self.account_id = QLineEdit(config.account_id)
        self.api_token = QLineEdit(config.api_token)
        self.api_token.setEchoMode(QLineEdit.EchoMode.Password)
        self.sync_interval = QSpinBox()
        self.sync_interval.setMinimum(1)
        self.sync_interval.setMaximum(1440)
        self.sync_interval.setValue(config.sync_interval_min)

        form.addRow("Account ID", self.account_id)
        form.addRow("API Token", self.api_token)
        form.addRow("Sync interval (min)", self.sync_interval)

        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Save | QDialogButtonBox.StandardButton.Cancel,
            Qt.Orientation.Horizontal,
            self,
        )
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)

        layout = QVBoxLayout()
        layout.addLayout(form)
        layout.addWidget(buttons)
        self.setLayout(layout)

    def get_config(self) -> Config:
        return Config(
            account_id=self.account_id.text().strip() or "U1234567",
            api_token=self.api_token.text().strip(),
            sync_interval_min=self.sync_interval.value(),
        )


class SyncWorker(QObject):
    progress = Signal(int, str)
    finished = Signal(list)
    failed = Signal(str)

    def __init__(self, declarations: list[dict], api_token: str) -> None:
        super().__init__()
        self.declarations = declarations
        self.api_token = api_token

    @Slot()
    def run(self) -> None:
        should_fail = "fail" in self.api_token.lower()

        progress = 0
        while progress < PROGRESS_MAX:
            time.sleep(0.35)
            progress = min(progress + PROGRESS_STEP, PROGRESS_MAX)
            self.progress.emit(progress, f"Syncing... {progress}%")

            if should_fail and progress >= PROGRESS_FAIL_AT:
                self.failed.emit("Connection to broker API timed out. Fix config and retry.")
                return

        updated = []
        for item in self.declarations:
            new_item = dict(item)
            if new_item["status"] == "Draft":
                new_item["status"] = "Submitted"
            updated.append(new_item)

        self.finished.emit(updated)


class MainWindow(QMainWindow):
    def __init__(self) -> None:  # noqa: PLR0915
        super().__init__()
        self.setWindowTitle("Declarations Sync MVP")
        self.resize(1080, 700)

        self.config = self.load_config()
        self.declarations: list[dict] = [
            {
                "id": "DEC-2026-001",
                "period": "Q1 2026",
                "status": "Active",
                "due_date": "2026-04-15",
                "amount_rsd": 18500,
            },
            {
                "id": "DEC-2026-002",
                "period": "Q2 2026",
                "status": "Draft",
                "due_date": "2026-07-15",
                "amount_rsd": 9300,
            },
            {
                "id": "DEC-2026-003",
                "period": "Q3 2026",
                "status": "Submitted",
                "due_date": "2026-10-15",
                "amount_rsd": 22120,
            },
        ]

        self.status_filter = "All"
        self.visible_indices: list[int] = []

        self.sync_thread: QThread | None = None
        self.sync_worker: SyncWorker | None = None

        root_widget = QWidget()
        root_widget.setObjectName("appRoot")
        root = QVBoxLayout(root_widget)
        root.setContentsMargins(16, 16, 16, 16)
        root.setSpacing(12)

        top_bar = QHBoxLayout()
        self.sync_button = QPushButton("↻  Sync")
        self.sync_button.setObjectName("syncButton")
        self.sync_button.setMinimumSize(220, 56)
        self.sync_button.clicked.connect(self.start_sync)

        self.config_button = QPushButton("Config")
        self.config_button.setObjectName("configButton")
        self.config_button.clicked.connect(self.open_config)

        top_bar.addWidget(self.sync_button)
        top_bar.addStretch(1)
        top_bar.addWidget(self.config_button)
        root.addLayout(top_bar)

        self.progress_label = QLabel("Idle")
        self.progress_label.setObjectName("progressLabel")
        self.progress_bar = QProgressBar()
        self.progress_bar.setObjectName("progressBar")
        self.progress_bar.setRange(0, PROGRESS_MAX)
        self.progress_bar.setValue(0)
        root.addWidget(self.progress_label)
        root.addWidget(self.progress_bar)

        title = QLabel("Declarations")
        title.setObjectName("tableTitle")
        subtitle = QLabel(
            "Filter by status, change one declaration or apply bulk action to selected rows",
        )
        subtitle.setObjectName("tableSubtitle")
        root.addWidget(title)
        root.addWidget(subtitle)

        filter_row = QHBoxLayout()
        filter_row.addWidget(QLabel("Filter status:"))
        self.filter_group = QButtonGroup(self)
        self.filter_group.setExclusive(True)
        for filter_name in ("All",) + STATUSES:
            button = QPushButton(filter_name)
            button.setObjectName("filterButton")
            button.setCheckable(True)
            if filter_name == "All":
                button.setChecked(True)
            button.clicked.connect(
                lambda _checked=False, value=filter_name: self.set_status_filter(value),
            )
            self.filter_group.addButton(button)
            filter_row.addWidget(button)
        filter_row.addStretch(1)
        root.addLayout(filter_row)

        controls_row = QHBoxLayout()
        self.select_all_button = QPushButton("Select all")
        self.select_submitted_button = QPushButton("Select submitted")
        self.clear_selection_button = QPushButton("Clear selection")
        self.bulk_active_button = QPushButton("Set Active")
        self.bulk_submitted_button = QPushButton("Set Submitted")
        self.bulk_draft_button = QPushButton("Set Draft")

        self.select_all_button.clicked.connect(self.select_all_rows)
        self.select_submitted_button.clicked.connect(self.select_submitted_rows)
        self.clear_selection_button.clicked.connect(self.clear_selection)
        self.bulk_active_button.clicked.connect(lambda: self.apply_status_to_selected("Active"))
        self.bulk_submitted_button.clicked.connect(
            lambda: self.apply_status_to_selected("Submitted"),
        )
        self.bulk_draft_button.clicked.connect(lambda: self.apply_status_to_selected("Draft"))

        controls_row.addWidget(self.select_all_button)
        controls_row.addWidget(self.select_submitted_button)
        controls_row.addWidget(self.clear_selection_button)
        controls_row.addStretch(1)
        controls_row.addWidget(self.bulk_active_button)
        controls_row.addWidget(self.bulk_submitted_button)
        controls_row.addWidget(self.bulk_draft_button)
        root.addLayout(controls_row)

        self.table = QTableWidget(0, 6)
        self.table.setHorizontalHeaderLabels(
            ["ID", "Period", "Status", "Due date", "Amount (RSD)", "Actions"],
        )
        self.table.verticalHeader().setVisible(False)
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.table.setAlternatingRowColors(True)

        header = self.table.horizontalHeader()
        header.setSectionResizeMode(0, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(4, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(5, QHeaderView.ResizeMode.Stretch)

        root.addWidget(self.table)

        self.setCentralWidget(root_widget)
        self.apply_theme()
        self.populate_table()

    def apply_theme(self) -> None:
        self.setStyleSheet(
            """
            QWidget#appRoot {
                background: #F4F7FB;
            }
            QLabel#tableTitle {
                font-size: 30px;
                font-weight: 800;
                color: #0F172A;
                margin-top: 4px;
            }
            QLabel#tableSubtitle {
                color: #64748B;
                margin-bottom: 4px;
            }
            QPushButton#syncButton {
                background-color: #0F766E;
                color: #FFFFFF;
                border: none;
                border-radius: 12px;
                font-size: 20px;
                font-weight: 700;
                padding: 8px 14px;
            }
            QPushButton#syncButton:hover {
                background-color: #0D9488;
            }
            QPushButton#configButton {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #CBD5E1;
                border-radius: 8px;
                padding: 8px 16px;
                font-weight: 600;
            }
            QPushButton#filterButton {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #CBD5E1;
                border-radius: 8px;
                padding: 6px 12px;
            }
            QPushButton#filterButton:checked {
                background: #DDEAFE;
                border: 1px solid #7BA8F4;
            }
            QLabel#progressLabel {
                color: #334155;
                font-weight: 600;
            }
            QProgressBar#progressBar {
                border: 1px solid #CBD5E1;
                border-radius: 7px;
                background: #E2E8F0;
                text-align: center;
                min-height: 16px;
            }
            QProgressBar#progressBar::chunk {
                background: #14B8A6;
                border-radius: 6px;
            }
            QTableWidget {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #D8E3F0;
                border-radius: 10px;
                gridline-color: #EDF2F7;
                alternate-background-color: #F8FBFF;
                selection-background-color: #E3F2FD;
                selection-color: #0F172A;
            }
            QHeaderView::section {
                background: #E8F0FA;
                color: #334155;
                border: none;
                border-bottom: 1px solid #D8E3F0;
                padding: 8px;
                font-weight: 700;
            }
            QToolButton {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #CBD5E1;
                border-radius: 6px;
                padding: 3px 8px;
            }
            QToolButton:hover {
                background: #F1F5F9;
            }
            """,
        )

    def set_status_filter(self, filter_name: str) -> None:
        self.status_filter = filter_name
        self.populate_table()

    def _visible_declaration_indices(self) -> list[int]:
        if self.status_filter == "All":
            return list(range(len(self.declarations)))
        return [
            index
            for index, declaration in enumerate(self.declarations)
            if declaration["status"] == self.status_filter
        ]

    def populate_table(self, reselect_source_rows: list[int] | None = None) -> None:
        self.visible_indices = self._visible_declaration_indices()
        self.table.setRowCount(len(self.visible_indices))

        for view_row, source_row in enumerate(self.visible_indices):
            declaration = self.declarations[source_row]
            self.table.setItem(view_row, 0, QTableWidgetItem(str(declaration["id"])))
            self.table.setItem(view_row, 1, QTableWidgetItem(str(declaration["period"])))
            self.table.setItem(view_row, 2, QTableWidgetItem(str(declaration["status"])))
            self.table.setItem(view_row, 3, QTableWidgetItem(str(declaration["due_date"])))
            self.table.setItem(view_row, 4, QTableWidgetItem(f"{declaration['amount_rsd']:,}"))
            self.table.setCellWidget(view_row, 5, self.build_row_actions(source_row))

        if reselect_source_rows:
            self.table.clearSelection()
            for source_row in reselect_source_rows:
                self.select_source_row(source_row)

    def build_row_actions(self, source_row: int) -> QWidget:
        wrapper = QWidget()
        layout = QHBoxLayout(wrapper)
        layout.setContentsMargins(2, 2, 2, 2)
        layout.setSpacing(6)

        for status in STATUSES:
            action_button = QToolButton()
            action_button.setText(status)
            action_button.clicked.connect(
                lambda _checked=False, row=source_row, target_status=status: self.set_row_status(
                    row,
                    target_status,
                ),
            )
            layout.addWidget(action_button)

        layout.addStretch(1)
        return wrapper

    def set_row_status(self, source_row: int, status: str) -> None:
        if source_row < 0 or source_row >= len(self.declarations):
            return
        self.declarations[source_row]["status"] = status
        self.populate_table(reselect_source_rows=[source_row])

    def selected_source_rows(self) -> list[int]:
        model = self.table.selectionModel()
        if model is None:
            return []

        selected_view_rows = sorted({index.row() for index in model.selectedRows()})
        return [
            self.visible_indices[view_row]
            for view_row in selected_view_rows
            if view_row < len(self.visible_indices)
        ]

    def select_source_row(self, source_row: int) -> None:
        if source_row not in self.visible_indices:
            return

        model = self.table.selectionModel()
        if model is None:
            return

        view_row = self.visible_indices.index(source_row)
        row_index = self.table.model().index(view_row, 0)
        model.select(
            row_index,
            QItemSelectionModel.SelectionFlag.Select | QItemSelectionModel.SelectionFlag.Rows,
        )

    @Slot()
    def select_all_rows(self) -> None:
        self.table.selectAll()

    @Slot()
    def select_submitted_rows(self) -> None:
        self.table.clearSelection()
        for source_row in self.visible_indices:
            if self.declarations[source_row]["status"] == "Submitted":
                self.select_source_row(source_row)

    @Slot()
    def clear_selection(self) -> None:
        self.table.clearSelection()

    def apply_status_to_selected(self, status: str) -> None:
        source_rows = self.selected_source_rows()
        if not source_rows:
            QMessageBox.information(self, "No selection", "Select one or more declarations first.")
            return

        for source_row in source_rows:
            self.declarations[source_row]["status"] = status

        self.populate_table(reselect_source_rows=source_rows)

    @Slot()
    def start_sync(self) -> None:
        if self.sync_thread is not None and self.sync_thread.isRunning():
            return

        self.progress_label.setText("Sync started")
        self.progress_bar.setValue(0)
        self.sync_button.setEnabled(False)

        self.sync_worker = SyncWorker(self.declarations, self.config.api_token)
        self.sync_thread = QThread(self)
        self.sync_worker.moveToThread(self.sync_thread)

        self.sync_thread.started.connect(self.sync_worker.run)
        self.sync_worker.progress.connect(self.on_sync_progress)
        self.sync_worker.finished.connect(self.on_sync_finished)
        self.sync_worker.failed.connect(self.on_sync_failed)

        self.sync_worker.finished.connect(self.sync_thread.quit)
        self.sync_worker.finished.connect(self.sync_worker.deleteLater)
        self.sync_worker.failed.connect(self.sync_thread.quit)
        self.sync_worker.failed.connect(self.sync_worker.deleteLater)

        self.sync_thread.finished.connect(self.on_sync_thread_finished)
        self.sync_thread.finished.connect(self.sync_thread.deleteLater)

        self.sync_thread.start()

    @Slot(int, str)
    def on_sync_progress(self, value: int, text: str) -> None:
        self.progress_bar.setValue(value)
        self.progress_label.setText(text)

    @Slot(list)
    def on_sync_finished(self, updated: list[dict]) -> None:
        self.declarations = updated
        self.populate_table()
        self.progress_label.setText("Sync complete")
        self.progress_bar.setValue(PROGRESS_MAX)

    @Slot(str)
    def on_sync_failed(self, message: str) -> None:
        self.progress_label.setText("Sync failed")
        QMessageBox.critical(self, "Sync error", message)

    @Slot()
    def on_sync_thread_finished(self) -> None:
        self.sync_button.setEnabled(True)
        self.sync_worker = None
        self.sync_thread = None

    @Slot()
    def open_config(self) -> None:
        dialog = ConfigDialog(self.config, self)
        if dialog.exec() == QDialog.DialogCode.Accepted:
            self.config = dialog.get_config()
            self.save_config(self.config)
            self.progress_label.setText("Config saved")

    def load_config(self) -> Config:
        if not CONFIG_PATH.exists():
            return Config()

        try:
            payload = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
            return Config(
                account_id=str(payload.get("account_id", "U1234567")),
                api_token=str(payload.get("api_token", "")),
                sync_interval_min=int(payload.get("sync_interval_min", 30)),
            )
        except (json.JSONDecodeError, OSError, ValueError):
            return Config()

    def save_config(self, config: Config) -> None:
        try:
            CONFIG_PATH.write_text(json.dumps(asdict(config), indent=2), encoding="utf-8")
        except OSError:
            self.progress_label.setText("Config save failed")


if __name__ == "__main__":
    app = QApplication([])
    window = MainWindow()
    window.show()
    app.exec()
