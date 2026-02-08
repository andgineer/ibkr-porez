from __future__ import annotations

import json
from dataclasses import asdict, dataclass
from pathlib import Path

from PySide6.QtCore import QItemSelectionModel, QObject, Qt, QThread, Signal, Slot
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QComboBox,
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

from ibkr_porez.config import config_manager
from ibkr_porez.declaration_manager import DeclarationManager
from ibkr_porez.error_handling import get_user_friendly_error_message
from ibkr_porez.models import Declaration, DeclarationStatus
from ibkr_porez.operation_sync import SyncOperation
from ibkr_porez.storage import Storage

CONFIG_PATH = Path.home() / ".ibkr-porez-mvp-config.json"
FILTER_ORDER = ("Active", "All", "Draft", "Submitted")
ROW_STATUS_ACTIONS = (
    ("To Submitted", "Submitted"),
    ("To paid", "Paid"),
    ("To draft", "Draft"),
)
PROGRESS_MAX = 100
INVALID_IDS_PREVIEW_COUNT = 3


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
    finished = Signal(int)
    failed = Signal(str)

    @Slot()
    def run(self) -> None:
        try:
            cfg = config_manager.load_config()
            if not cfg.ibkr_token or not cfg.ibkr_query_id:
                self.failed.emit(
                    "Missing IBKR configuration. Run `ibkr-porez config` first.",
                )
                return
            operation = SyncOperation(cfg)
            created = operation.execute()
            self.finished.emit(len(created))
        except Exception as e:  # noqa: BLE001
            self.failed.emit(get_user_friendly_error_message(e))


class MainWindow(QMainWindow):
    def __init__(self) -> None:  # noqa: PLR0915
        super().__init__()
        self.setWindowTitle("Declarations Sync MVP")
        self.resize(1080, 700)

        self.config = self.load_config()
        self.storage = Storage()
        self.declaration_manager = DeclarationManager()
        self.declarations: list[Declaration] = []
        self.reload_declarations()

        self.status_filter = "Active"
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

        self.progress_label = QLabel("")
        self.progress_label.setObjectName("progressLabel")
        self.progress_bar = QProgressBar()
        self.progress_bar.setObjectName("progressBar")
        self.progress_bar.setRange(0, PROGRESS_MAX)
        self.progress_bar.setValue(0)
        root.addWidget(self.progress_label)
        root.addWidget(self.progress_bar)

        action_bar = QHBoxLayout()
        action_bar.setSpacing(8)
        self.filter_combo = QComboBox()
        self.filter_combo.setObjectName("filterCombo")
        self.filter_combo.addItems(FILTER_ORDER)
        self.filter_combo.setCurrentText(self.status_filter)
        self.filter_combo.currentTextChanged.connect(self.set_status_filter)
        action_bar.addWidget(self.filter_combo)

        action_bar.addStretch(1)
        self.select_all_button = QPushButton("Select all")
        self.clear_selection_button = QPushButton("Unselect all")
        self.selection_info_label = QLabel("0 selected")
        self.selection_info_label.setObjectName("selectionInfoLabel")
        self.select_all_button.setObjectName("controlButton")
        self.clear_selection_button.setObjectName("controlButton")
        self.select_all_button.setMinimumWidth(90)
        self.clear_selection_button.setMinimumWidth(90)

        self.select_all_button.clicked.connect(self.select_all_rows)
        self.clear_selection_button.clicked.connect(self.clear_selection)
        action_bar.addWidget(self.select_all_button)
        action_bar.addWidget(self.clear_selection_button)
        action_bar.addWidget(self.selection_info_label)
        self.bulk_status_combo = QComboBox()
        self.bulk_status_combo.addItems(["Draft", "Submitted", "Paid"])
        self.bulk_status_combo.setObjectName("controlCombo")
        self.apply_status_button = QPushButton("Apply to selected")
        self.apply_status_button.setObjectName("primaryControlButton")
        self.apply_status_button.setMinimumWidth(180)
        self.bulk_status_combo.setVisible(False)
        self.apply_status_button.setVisible(False)
        self.apply_status_button.clicked.connect(self.apply_selected_status_from_combo)
        action_bar.addWidget(self.bulk_status_combo)
        action_bar.addWidget(self.apply_status_button)
        root.addLayout(action_bar)

        self.table = QTableWidget(0, 6)
        self.table.setHorizontalHeaderLabels(
            [
                "Declaration ID",
                "Type",
                "Period",
                "Status",
                "Created",
                "Actions",
            ],
        )
        self.table.verticalHeader().setVisible(False)
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.ExtendedSelection)
        self.table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.table.setAlternatingRowColors(True)
        self.table.itemSelectionChanged.connect(self.update_selection_info)

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
            QComboBox#filterCombo {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #CBD5E1;
                border-radius: 8px;
                padding: 4px 26px 4px 8px;
                min-width: 130px;
            }
            QComboBox#filterCombo::drop-down {
                subcontrol-origin: padding;
                subcontrol-position: top right;
                width: 24px;
                border-left: 1px solid #CBD5E1;
                background: #FFFFFF;
                border-top-right-radius: 8px;
                border-bottom-right-radius: 8px;
            }
            QComboBox#filterCombo QAbstractItemView {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #CBD5E1;
                selection-background-color: #E3F2FD;
                selection-color: #0F172A;
                outline: 0;
            }
            QPushButton#controlButton {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #CBD5E1;
                border-radius: 8px;
                padding: 6px 12px;
                font-weight: 600;
            }
            QPushButton#controlButton:hover {
                background: #F8FAFC;
            }
            QPushButton#primaryControlButton {
                background-color: #0F766E;
                color: #FFFFFF;
                border: none;
                border-radius: 8px;
                padding: 8px 14px;
                font-weight: 700;
            }
            QPushButton#primaryControlButton:disabled {
                background-color: #94A3B8;
                color: #E2E8F0;
            }
            QComboBox#controlCombo {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #CBD5E1;
                border-radius: 8px;
                padding: 4px 26px 4px 8px;
                min-width: 140px;
            }
            QComboBox#controlCombo::drop-down {
                subcontrol-origin: padding;
                subcontrol-position: top right;
                width: 24px;
                border-left: 1px solid #CBD5E1;
                background: #FFFFFF;
                border-top-right-radius: 8px;
                border-bottom-right-radius: 8px;
            }
            QComboBox#controlCombo QAbstractItemView {
                background: #FFFFFF;
                color: #0F172A;
                border: 1px solid #CBD5E1;
                selection-background-color: #E3F2FD;
                selection-color: #0F172A;
                outline: 0;
            }
            QLabel#selectionInfoLabel {
                color: #475569;
                font-weight: 600;
                padding-left: 6px;
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

    def reload_declarations(self) -> None:
        declarations = self.storage.get_declarations()
        declarations.sort(key=lambda d: d.created_at, reverse=True)
        self.declarations = declarations

    def _status_label(self, status: DeclarationStatus) -> str:
        return status.value.capitalize()

    def _status_from_label(self, label: str) -> DeclarationStatus:
        return DeclarationStatus(label.lower())

    def _file_name(self, declaration: Declaration) -> str:
        if declaration.file_path:
            return Path(declaration.file_path).name
        return f"{declaration.declaration_id}.xml"

    def _period_text(self, declaration: Declaration) -> str:
        return f"{declaration.period_start} to {declaration.period_end}"

    def _is_transition_allowed(
        self,
        current_status: DeclarationStatus,
        target_status: DeclarationStatus,
    ) -> bool:
        if current_status == target_status:
            return False
        if target_status == DeclarationStatus.SUBMITTED:
            return current_status == DeclarationStatus.DRAFT
        if target_status == DeclarationStatus.PAID:
            return current_status in {DeclarationStatus.DRAFT, DeclarationStatus.SUBMITTED}
        if target_status == DeclarationStatus.DRAFT:
            return current_status in {DeclarationStatus.SUBMITTED, DeclarationStatus.PAID}
        return False

    def set_status_filter(self, filter_name: str) -> None:
        self.status_filter = filter_name
        self.populate_table()

    def _visible_declaration_indices(self) -> list[int]:
        if self.status_filter == "All":
            return list(range(len(self.declarations)))
        if self.status_filter == "Active":
            return [
                index
                for index, declaration in enumerate(self.declarations)
                if declaration.status in {DeclarationStatus.DRAFT, DeclarationStatus.SUBMITTED}
            ]
        status_enum = self._status_from_label(self.status_filter)
        return [
            index
            for index, declaration in enumerate(self.declarations)
            if declaration.status == status_enum
        ]

    def populate_table(self, reselect_declaration_ids: list[str] | None = None) -> None:
        self.visible_indices = self._visible_declaration_indices()
        self.table.setRowCount(len(self.visible_indices))

        for view_row, source_row in enumerate(self.visible_indices):
            declaration = self.declarations[source_row]
            self.table.setItem(view_row, 0, QTableWidgetItem(declaration.declaration_id))
            self.table.setItem(view_row, 1, QTableWidgetItem(declaration.type.value))
            self.table.setItem(view_row, 2, QTableWidgetItem(self._period_text(declaration)))
            self.table.setItem(
                view_row,
                3,
                QTableWidgetItem(self._status_label(declaration.status)),
            )
            self.table.setItem(
                view_row,
                4,
                QTableWidgetItem(declaration.created_at.strftime("%Y-%m-%d")),
            )
            self.table.setCellWidget(view_row, 5, self.build_row_actions(source_row))

        if reselect_declaration_ids:
            self.table.clearSelection()
            for declaration_id in reselect_declaration_ids:
                self.select_declaration_row(declaration_id)
        self.update_selection_info()

    def build_row_actions(self, source_row: int) -> QWidget:
        wrapper = QWidget()
        layout = QHBoxLayout(wrapper)
        layout.setContentsMargins(2, 2, 2, 2)
        layout.setSpacing(6)

        current_status = self.declarations[source_row].status
        for label, status in ROW_STATUS_ACTIONS:
            target_status = self._status_from_label(status)
            if not self._is_transition_allowed(current_status, target_status):
                continue
            action_button = QToolButton()
            action_button.setText(label)
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
        declaration_id = self.declarations[source_row].declaration_id
        try:
            self.apply_status_to_ids([declaration_id], status)
            self.reload_declarations()
            self.populate_table(reselect_declaration_ids=[declaration_id])
        except ValueError as e:
            QMessageBox.warning(self, "Status change failed", str(e))

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

    def selected_declaration_ids(self) -> list[str]:
        return [self.declarations[row].declaration_id for row in self.selected_source_rows()]

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

    def select_declaration_row(self, declaration_id: str) -> None:
        for source_row in self.visible_indices:
            if self.declarations[source_row].declaration_id == declaration_id:
                self.select_source_row(source_row)
                break

    @Slot()
    def select_all_rows(self) -> None:
        self.table.selectAll()

    @Slot()
    def clear_selection(self) -> None:
        self.table.clearSelection()

    @Slot()
    def update_selection_info(self) -> None:
        selected_count = len(self.selected_source_rows())
        self.selection_info_label.setText(f"{selected_count} selected")
        has_selection = selected_count > 0
        self.bulk_status_combo.setVisible(has_selection)
        self.apply_status_button.setVisible(has_selection)
        self.apply_status_button.setEnabled(has_selection)

    @Slot()
    def apply_selected_status_from_combo(self) -> None:
        self.apply_status_to_selected(self.bulk_status_combo.currentText())

    def apply_status_to_ids(self, declaration_ids: list[str], status: str) -> None:
        target_status = self._status_from_label(status)
        selected_map = {d.declaration_id: d for d in self.declarations}
        invalid_ids = []
        for declaration_id in declaration_ids:
            declaration = selected_map.get(declaration_id)
            if declaration is None or not self._is_transition_allowed(
                declaration.status,
                target_status,
            ):
                invalid_ids.append(declaration_id)

        if invalid_ids:
            invalid_sample = ", ".join(invalid_ids[:INVALID_IDS_PREVIEW_COUNT])
            suffix = "..." if len(invalid_ids) > INVALID_IDS_PREVIEW_COUNT else ""
            raise ValueError(
                f"Status change to {status} is not allowed for: {invalid_sample}{suffix}",
            )

        if target_status == DeclarationStatus.SUBMITTED:
            self.declaration_manager.submit(declaration_ids)
            return
        if target_status == DeclarationStatus.PAID:
            self.declaration_manager.pay(declaration_ids)
            return
        if target_status == DeclarationStatus.DRAFT:
            self.declaration_manager.revert(declaration_ids, DeclarationStatus.DRAFT)
            return
        raise ValueError(f"Unsupported status: {status}")

    def apply_status_to_selected(self, status: str) -> None:
        declaration_ids = self.selected_declaration_ids()
        if not declaration_ids:
            QMessageBox.information(self, "No selection", "Select one or more declarations first.")
            return

        try:
            self.apply_status_to_ids(declaration_ids, status)
            self.reload_declarations()
            self.populate_table(reselect_declaration_ids=declaration_ids)
            self.statusBar().showMessage(
                f"Updated {len(declaration_ids)} declarations to {status}",
                4000,
            )
        except ValueError as e:
            QMessageBox.warning(self, "Bulk update failed", str(e))

    @Slot()
    def start_sync(self) -> None:
        if self.sync_thread is not None and self.sync_thread.isRunning():
            return

        self.progress_label.setText("Sync started")
        self.progress_bar.setRange(0, 0)
        self.sync_button.setEnabled(False)

        self.sync_worker = SyncWorker()
        self.sync_thread = QThread(self)
        self.sync_worker.moveToThread(self.sync_thread)

        self.sync_thread.started.connect(self.sync_worker.run)
        self.sync_worker.finished.connect(self.on_sync_finished)
        self.sync_worker.failed.connect(self.on_sync_failed)

        self.sync_worker.finished.connect(self.sync_thread.quit)
        self.sync_worker.finished.connect(self.sync_worker.deleteLater)
        self.sync_worker.failed.connect(self.sync_thread.quit)
        self.sync_worker.failed.connect(self.sync_worker.deleteLater)

        self.sync_thread.finished.connect(self.on_sync_thread_finished)
        self.sync_thread.finished.connect(self.sync_thread.deleteLater)

        self.sync_thread.start()

    @Slot(int)
    def on_sync_finished(self, created_count: int) -> None:
        self.reload_declarations()
        self.populate_table()
        self.progress_label.setText(f"Sync complete, created {created_count} declaration(s)")
        self.progress_bar.setRange(0, PROGRESS_MAX)
        self.progress_bar.setValue(PROGRESS_MAX)

    @Slot(str)
    def on_sync_failed(self, message: str) -> None:
        self.progress_label.setText("Sync failed")
        self.progress_bar.setRange(0, PROGRESS_MAX)
        self.progress_bar.setValue(0)
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
    app.setStyle("Fusion")
    window = MainWindow()
    window.show()
    app.exec()
