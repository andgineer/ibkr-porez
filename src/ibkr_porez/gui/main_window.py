from __future__ import annotations

from decimal import Decimal, InvalidOperation

from PySide6.QtCore import QItemSelectionModel, Qt, QThread, Slot
from PySide6.QtGui import QAction, QKeySequence, QPalette, QShortcut
from PySide6.QtWidgets import (
    QAbstractItemView,
    QComboBox,
    QDialog,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QMainWindow,
    QMenu,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QToolButton,
    QVBoxLayout,
    QWidget,
)

from ibkr_porez.config import config_manager, get_data_dir_change_warning
from ibkr_porez.declaration_manager import DeclarationManager
from ibkr_porez.gui.assessment_dialog import AssessmentDialog
from ibkr_porez.gui.config_dialog import ConfigDialog
from ibkr_porez.gui.constants import (
    BULK_STATUS_OPTIONS,
    FILTER_ORDER,
    INVALID_IDS_PREVIEW_COUNT,
    PROGRESS_MAX,
    ROW_STATUS_ACTIONS,
)
from ibkr_porez.gui.declaration_details_dialog import DeclarationDetailsDialog
from ibkr_porez.gui.export_worker import ExportWorker
from ibkr_porez.gui.import_dialog import ImportDialog
from ibkr_porez.gui.styles import APP_STYLESHEET
from ibkr_porez.gui.sync_worker import SyncWorker
from ibkr_porez.models import Declaration, DeclarationStatus
from ibkr_porez.storage import Storage

EMPTY_TRANSACTIONS_WARNING = (
    "Transaction history is empty. If your trading history is over one year, import it via "
    "Import. This is important for correct stock sale tax calculation."
)


class MainWindow(QMainWindow):
    def __init__(self) -> None:  # noqa: PLR0915
        super().__init__()
        self.setWindowTitle("ibkr-porez")
        self.resize(1080, 700)

        self.config = config_manager.load_config()
        self.storage = Storage()
        self.declaration_manager = DeclarationManager()
        self.declarations: list[Declaration] = []
        self.reload_declarations()

        self.status_filter = "Active"
        self.visible_indices: list[int] = []

        self.sync_thread: QThread | None = None
        self.sync_worker: SyncWorker | None = None
        self.export_thread: QThread | None = None
        self.export_worker: ExportWorker | None = None
        self.export_active_button: QToolButton | None = None
        self.export_prev_progress: tuple[int, int, int] | None = None

        root_widget = QWidget()
        root_widget.setObjectName("appRoot")
        root = QVBoxLayout(root_widget)
        root.setContentsMargins(16, 16, 16, 16)
        root.setSpacing(12)

        top_bar = QHBoxLayout()
        self.sync_button = QToolButton()
        self.sync_button.setText("↻  Sync")
        self.sync_button.setObjectName("syncButton")
        self.sync_button.setMinimumSize(220, 56)
        self.sync_button.setPopupMode(QToolButton.ToolButtonPopupMode.MenuButtonPopup)
        self.sync_button.clicked.connect(self.start_sync)
        self.sync_menu = QMenu(self.sync_button)
        self.sync_menu.setToolTipsVisible(True)
        self.force_sync_action = QAction("Force sync", self.sync_menu)
        self.force_sync_action.setToolTip(
            "Ignore last sync date, rescan recent history, and create declarations even if "
            "withholding tax is not found.",
        )
        self.force_sync_action.triggered.connect(lambda _checked=False: self.start_forced_sync())
        self.sync_menu.addAction(self.force_sync_action)
        self.sync_button.setMenu(self.sync_menu)

        self.config_button = QPushButton("Config")
        self.config_button.setObjectName("configButton")
        self.config_button.clicked.connect(self.open_config)

        self.import_button = QPushButton("Import")
        self.import_button.setObjectName("configButton")
        self.import_button.clicked.connect(self.open_import)

        top_bar.addWidget(self.sync_button)
        top_bar.addStretch(1)
        top_bar.addWidget(self.import_button)
        top_bar.addWidget(self.config_button)
        root.addLayout(top_bar)

        self.progress_label = QLabel("")
        self.progress_label.setObjectName("progressLabel")
        self.progress_label.setWordWrap(True)
        self.progress_bar = QProgressBar()
        self.progress_bar.setObjectName("progressBar")
        self.progress_bar.setRange(0, PROGRESS_MAX)
        self.progress_bar.setValue(0)
        self.progress_bar.setVisible(False)
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
        self.bulk_status_combo.addItems(BULK_STATUS_OPTIONS)
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

        self.table = QTableWidget(0, 7)
        self.table.setHorizontalHeaderLabels(
            [
                "ID",
                "Type",
                "Period",
                "Tax",
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
        self.table.itemDoubleClicked.connect(self.open_declaration_details_for_item)
        self.open_details_shortcut_return = QShortcut(QKeySequence("Return"), self.table)
        self.open_details_shortcut_return.activated.connect(
            self.open_declaration_details_for_selected_row,
        )
        self.open_details_shortcut_enter = QShortcut(QKeySequence("Enter"), self.table)
        self.open_details_shortcut_enter.activated.connect(
            self.open_declaration_details_for_selected_row,
        )

        header = self.table.horizontalHeader()
        header.setSectionResizeMode(0, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(4, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(5, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(6, QHeaderView.ResizeMode.Stretch)

        root.addWidget(self.table)
        self.details_hint_label = QLabel("Tip: double-click a row or press Enter to open details.")
        hint_palette = self.details_hint_label.palette()
        hint_palette.setColor(
            QPalette.ColorRole.WindowText,
            hint_palette.color(QPalette.ColorRole.PlaceholderText),
        )
        self.details_hint_label.setPalette(hint_palette)
        root.addWidget(self.details_hint_label, alignment=Qt.AlignmentFlag.AlignRight)

        self.setCentralWidget(root_widget)
        self.setStyleSheet(APP_STYLESHEET)
        self.populate_table()
        self._update_empty_transactions_warning(force=True)

    def reload_declarations(self) -> None:
        declarations = self.storage.get_declarations()
        declarations.sort(key=lambda d: d.created_at, reverse=True)
        self.declarations = declarations

    @staticmethod
    def _status_label(status: DeclarationStatus) -> str:
        return status.value.replace("-", " ").replace("_", " ").title()

    @staticmethod
    def _status_from_label(label: str) -> DeclarationStatus:
        return DeclarationStatus(label.lower())

    @staticmethod
    def _status_from_action(action: str) -> str:
        if action == "Submit":
            return "Submitted"
        if action == "Pay":
            return "Finalized"
        return action

    @staticmethod
    def _is_transition_allowed(
        current_status: DeclarationStatus,
        target_status: DeclarationStatus,
    ) -> bool:
        return DeclarationManager.is_transition_allowed(current_status, target_status)

    @staticmethod
    def _tax_due_from_metadata(declaration: Declaration) -> Decimal | None:
        for key in ("assessed_tax_due_rsd", "tax_due_rsd"):
            value = declaration.metadata.get(key)
            if value is None:
                continue
            try:
                return Decimal(str(value)).quantize(Decimal("0.01"))
            except (InvalidOperation, TypeError, ValueError):
                continue
        return None

    def _visible_declaration_indices(self) -> list[int]:
        if self.status_filter == "All":
            return list(range(len(self.declarations)))
        if self.status_filter == "Active":
            return [
                index
                for index, declaration in enumerate(self.declarations)
                if declaration.status
                in {
                    DeclarationStatus.DRAFT,
                    DeclarationStatus.SUBMITTED,
                    DeclarationStatus.PENDING,
                }
            ]
        if self.status_filter == "Pending payment":
            return [
                index
                for index, declaration in enumerate(self.declarations)
                if declaration.status in {DeclarationStatus.SUBMITTED, DeclarationStatus.PENDING}
            ]
        status_enum = self._status_from_label(self.status_filter)
        return [
            index
            for index, declaration in enumerate(self.declarations)
            if declaration.status == status_enum
        ]

    def set_status_filter(self, filter_name: str) -> None:
        self.status_filter = filter_name
        self.populate_table()

    def populate_table(self, reselect_declaration_ids: list[str] | None = None) -> None:
        self.visible_indices = self._visible_declaration_indices()
        self.table.setRowCount(len(self.visible_indices))

        for view_row, source_row in enumerate(self.visible_indices):
            declaration = self.declarations[source_row]
            self.table.setItem(view_row, 0, QTableWidgetItem(declaration.declaration_id))
            self.table.setItem(
                view_row,
                1,
                QTableWidgetItem(declaration.display_type()),
            )
            self.table.setItem(
                view_row,
                2,
                QTableWidgetItem(declaration.display_period()),
            )
            self.table.setItem(
                view_row,
                3,
                QTableWidgetItem(declaration.display_tax()),
            )
            self.table.setItem(
                view_row,
                4,
                QTableWidgetItem(self._status_label(declaration.status)),
            )
            self.table.setItem(
                view_row,
                5,
                QTableWidgetItem(declaration.created_at.strftime("%Y-%m-%d")),
            )
            self.table.setCellWidget(view_row, 6, self.build_row_actions(source_row))

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
            if current_status == DeclarationStatus.FINALIZED and label in {"Submit", "Pay"}:
                continue
            if current_status != DeclarationStatus.FINALIZED and label == "Revert":
                continue
            target_status = self._status_from_label(status)
            action_button = QToolButton()
            action_button.setObjectName("statusActionButton")
            action_button.setText(label)
            allowed = self._is_transition_allowed(current_status, target_status)
            if (
                allowed
                and target_status == DeclarationStatus.FINALIZED
                and not self.declaration_manager.has_tax_to_pay(self.declarations[source_row])
            ):
                allowed = False
                action_button.setToolTip("No tax to pay. Use Submit to finalize declaration.")
            action_button.setEnabled(allowed)
            if not allowed and not action_button.toolTip():
                action_button.setToolTip("Status transition not allowed")
            action_button.clicked.connect(
                lambda _checked=False, row=source_row, target_status=status: self.set_row_status(
                    row,
                    target_status,
                ),
            )
            layout.addWidget(action_button)

        if current_status != DeclarationStatus.DRAFT:
            assess_button = QToolButton()
            assess_button.setObjectName("statusActionButton")
            assess_button.setText("Set tax")
            assess_button.setToolTip("Set official assessed tax amount")
            assess_button.clicked.connect(
                lambda _checked=False, row=source_row: self.open_assessment_dialog(row),
            )
            layout.addWidget(assess_button)

        export_button = QToolButton()
        export_button.setObjectName("exportActionButton")
        export_button.setText("Re-export")
        export_button.setCheckable(True)
        export_button.setToolTip("Save declaration files")
        export_button.clicked.connect(
            lambda _checked=False, row=source_row, btn=export_button: self.re_export_declaration(
                row,
                btn,
            ),
        )
        layout.addWidget(export_button)

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

    def open_assessment_dialog(self, source_row: int) -> None:
        if source_row < 0 or source_row >= len(self.declarations):
            return
        declaration = self.declarations[source_row]
        dialog = AssessmentDialog(
            declaration_id=declaration.declaration_id,
            initial_tax_due_rsd=self._tax_due_from_metadata(declaration),
            parent=self,
        )
        if dialog.exec() != QDialog.DialogCode.Accepted or dialog.tax_due_rsd is None:
            return

        try:
            updated = self.declaration_manager.set_assessed_tax(
                declaration_id=declaration.declaration_id,
                tax_due_rsd=dialog.tax_due_rsd,
                mark_paid=dialog.mark_paid,
            )
            self.reload_declarations()
            self.populate_table(reselect_declaration_ids=[declaration.declaration_id])
            if dialog.mark_paid:
                self._show_command_success(
                    f"Assessment saved and paid: {declaration.declaration_id} "
                    f"({dialog.tax_due_rsd} RSD)",
                )
            elif updated.status == DeclarationStatus.FINALIZED:
                self._show_command_success(
                    f"Assessment saved: {declaration.declaration_id} (no tax to pay)",
                )
            else:
                self._show_command_success(
                    f"Assessment saved: {declaration.declaration_id} "
                    f"({dialog.tax_due_rsd} RSD to pay)",
                )
        except ValueError as e:
            self._show_command_error("Assessment error", str(e), "Assessment update failed")

    def re_export_declaration(self, source_row: int, button: QToolButton | None = None) -> None:
        if self.export_thread is not None and self.export_thread.isRunning():
            return
        if source_row < 0 or source_row >= len(self.declarations):
            return

        self.progress_label.setText("")
        prev_min = self.progress_bar.minimum()
        prev_max = self.progress_bar.maximum()
        prev_value = self.progress_bar.value()
        self.export_prev_progress = (prev_min, prev_max, prev_value)
        self.progress_bar.setRange(0, 0)

        if button is not None:
            self.export_active_button = button
        self._set_export_buttons_busy_state(self.export_active_button)
        declaration_id = self.declarations[source_row].declaration_id
        self.export_worker = ExportWorker(declaration_id)
        self.export_thread = QThread(self)
        self.export_worker.moveToThread(self.export_thread)

        self.export_thread.started.connect(self.export_worker.run)
        self.export_worker.finished.connect(self.on_export_finished)
        self.export_worker.failed.connect(self.on_export_failed)
        self.export_worker.finished.connect(self.export_thread.quit)
        self.export_worker.finished.connect(self.export_worker.deleteLater)
        self.export_worker.failed.connect(self.export_thread.quit)
        self.export_worker.failed.connect(self.export_worker.deleteLater)
        self.export_thread.finished.connect(self.on_export_thread_finished)
        self.export_thread.finished.connect(self.export_thread.deleteLater)
        self.export_thread.start()

    def _set_export_buttons_busy_state(self, active_button: QToolButton | None) -> None:
        for view_row in range(self.table.rowCount()):
            row_widget = self.table.cellWidget(view_row, 6)
            if row_widget is None:
                continue
            for tool_button in row_widget.findChildren(QToolButton):
                if tool_button.objectName() == "exportActionButton":
                    is_active = active_button is not None and tool_button is active_button
                    tool_button.setChecked(is_active)
                    tool_button.setEnabled(active_button is None or is_active)

    def _finish_export_ui_state(self) -> None:
        if self.export_prev_progress is not None:
            prev_min, prev_max, prev_value = self.export_prev_progress
            self.progress_bar.setRange(prev_min, prev_max)
            self.progress_bar.setValue(prev_value)
            self.export_prev_progress = None

        self._set_export_buttons_busy_state(None)
        self.export_active_button = None

    def _show_command_success(self, message: str) -> None:
        self.progress_label.setText(message)

    def _show_command_error(self, title: str, message: str, summary: str) -> None:
        self.progress_label.setText(summary)
        QMessageBox.critical(self, title, message)

    def _is_transactions_history_empty(self) -> bool:
        return self.storage.get_last_transaction_date() is None

    def _update_empty_transactions_warning(self, force: bool = False) -> None:
        if self._is_transactions_history_empty():
            if force or not self.progress_label.text():
                self.progress_label.setText(EMPTY_TRANSACTIONS_WARNING)
            return

        if self.progress_label.text() == EMPTY_TRANSACTIONS_WARNING:
            self.progress_label.setText("")

    def _confirm_sync_with_empty_transactions(self) -> bool:
        confirm = QMessageBox(self)
        confirm.setIcon(QMessageBox.Icon.Warning)
        confirm.setWindowTitle("Transaction history is empty")
        confirm.setText(EMPTY_TRANSACTIONS_WARNING)
        confirm.setInformativeText("Continue sync without imported history?")
        confirm.setStandardButtons(
            QMessageBox.StandardButton.Ok | QMessageBox.StandardButton.Cancel,
        )
        continue_button = confirm.button(QMessageBox.StandardButton.Ok)
        if continue_button is not None:
            continue_button.setText("Continue")
        cancel_button = confirm.button(QMessageBox.StandardButton.Cancel)
        if cancel_button is not None:
            cancel_button.setText("Cancel")
        confirm.setDefaultButton(QMessageBox.StandardButton.Cancel)
        return confirm.exec() == int(QMessageBox.StandardButton.Ok)

    @Slot(str)
    def on_export_finished(self, export_dir: str) -> None:
        self._show_command_success(f"Declaration files saved in {export_dir}")
        self._finish_export_ui_state()

    @Slot(str)
    def on_export_failed(self, message: str) -> None:
        self._finish_export_ui_state()
        self._show_command_error("Export error", message, "Re-export failed")

    @Slot()
    def on_export_thread_finished(self) -> None:
        self.export_worker = None
        self.export_thread = None

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

    @Slot(QTableWidgetItem)
    def open_declaration_details_for_item(self, item: QTableWidgetItem) -> None:
        self.open_declaration_details_by_view_row(item.row())

    @Slot()
    def open_declaration_details_for_selected_row(self) -> None:
        model = self.table.selectionModel()
        if model is None:
            return

        selected_rows = [index.row() for index in model.selectedRows()]
        if selected_rows:
            self.open_declaration_details_by_view_row(min(selected_rows))
            return

        current_row = self.table.currentRow()
        if current_row >= 0:
            self.open_declaration_details_by_view_row(current_row)

    def open_declaration_details_by_view_row(self, view_row: int) -> None:
        if view_row < 0 or view_row >= len(self.visible_indices):
            return
        source_row = self.visible_indices[view_row]
        if source_row < 0 or source_row >= len(self.declarations):
            return
        declaration_id = self.declarations[source_row].declaration_id
        dialog = DeclarationDetailsDialog(declaration_id, self)
        dialog.exec()

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
        self.apply_status_to_selected(
            self._status_from_action(self.bulk_status_combo.currentText()),
        )

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
                continue
            if (
                target_status == DeclarationStatus.FINALIZED
                and not self.declaration_manager.has_tax_to_pay(declaration)
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
        if target_status == DeclarationStatus.FINALIZED:
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
        self._start_sync(forced=False)

    @Slot()
    def start_forced_sync(self) -> None:
        if not self._confirm_forced_sync():
            return
        self._start_sync(forced=True)

    def _start_sync(self, forced: bool) -> None:
        if self.sync_thread is not None and self.sync_thread.isRunning():
            return

        if (
            self._is_transactions_history_empty()
            and not self._confirm_sync_with_empty_transactions()
        ):
            self._update_empty_transactions_warning(force=True)
            return

        self.progress_label.setText("Forced sync started" if forced else "Sync started")
        self.progress_bar.setVisible(True)
        self.progress_bar.setRange(0, 0)
        self.sync_button.setEnabled(False)

        self.sync_worker = SyncWorker(forced=forced)
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

    def _confirm_forced_sync(self) -> bool:
        confirm = QMessageBox(self)
        confirm.setIcon(QMessageBox.Icon.Warning)
        confirm.setWindowTitle("Force sync")
        confirm.setText(
            "Force sync ignores last sync date and can create declarations even when "
            "withholding tax is not found.",
        )
        confirm.setInformativeText("Continue?")
        confirm.setStandardButtons(
            QMessageBox.StandardButton.Ok | QMessageBox.StandardButton.Cancel,
        )
        continue_button = confirm.button(QMessageBox.StandardButton.Ok)
        if continue_button is not None:
            continue_button.setText("Continue")
        cancel_button = confirm.button(QMessageBox.StandardButton.Cancel)
        if cancel_button is not None:
            cancel_button.setText("Cancel")
        confirm.setDefaultButton(QMessageBox.StandardButton.Cancel)
        return confirm.exec() == int(QMessageBox.StandardButton.Ok)

    @Slot(int, str)
    def on_sync_finished(self, created_count: int, output_folder: str) -> None:
        self.reload_declarations()
        self.populate_table()
        if created_count == 0:
            self._show_command_success("Sync complete, no new declarations")
        else:
            self._show_command_success(
                f"Sync complete, created {created_count} declaration(s) in {output_folder}",
            )
        self.progress_bar.setRange(0, PROGRESS_MAX)
        self.progress_bar.setValue(PROGRESS_MAX)

    @Slot(str)
    def on_sync_failed(self, message: str) -> None:
        self.progress_bar.setRange(0, PROGRESS_MAX)
        self.progress_bar.setValue(0)
        self._show_command_error("Sync error", message, "Sync failed")

    @Slot()
    def on_sync_thread_finished(self) -> None:
        self.progress_bar.setVisible(False)
        self.sync_button.setEnabled(True)
        self.sync_worker = None
        self.sync_thread = None

    @Slot()
    def open_config(self) -> None:
        dialog = ConfigDialog(self.config, self)
        if dialog.exec() == QDialog.DialogCode.Accepted:
            new_config = dialog.get_config()
            data_dir_warning = get_data_dir_change_warning(self.config, new_config)
            try:
                config_manager.save_config(new_config)
                self.config = new_config
                self.storage = Storage()
                self.reload_declarations()
                self.populate_table()
                self._update_empty_transactions_warning(force=True)
                self.progress_label.setText("Config saved")
                if data_dir_warning:
                    QMessageBox.warning(self, "Data directory changed", data_dir_warning)
            except OSError:
                self.progress_label.setText("Config save failed")

    @Slot()
    def open_import(self) -> None:
        dialog = ImportDialog(self)
        dialog.exec()
        self._update_empty_transactions_warning(force=True)
