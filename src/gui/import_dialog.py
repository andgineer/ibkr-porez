from __future__ import annotations

from pathlib import Path
from typing import override

from PySide6.QtCore import Qt, QThread, Slot
from PySide6.QtGui import QCloseEvent
from PySide6.QtWidgets import (
    QComboBox,
    QDialog,
    QFileDialog,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QProgressBar,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from gui.import_worker import ImportWorker
from ibkr_porez.operation_import import ImportType

IMPORT_DOCS_URL = (
    "https://andgineer.github.io/ibkr-porez/ibkr/#export-full-history-for-import-command"
)


class ImportDialog(QDialog):
    def __init__(self, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Import Transactions")
        self.setModal(True)
        self.resize(720, 360)

        self.import_thread: QThread | None = None
        self.import_worker: ImportWorker | None = None

        self._create_widgets()

        layout = QVBoxLayout()
        layout.setSpacing(12)
        layout.addLayout(self._build_form())
        layout.addWidget(self.progress_bar)
        layout.addWidget(self.error_label)
        layout.addWidget(self.result_label)
        layout.addStretch(1)
        layout.addLayout(self._build_buttons_row())
        self.setLayout(layout)

    def _create_widgets(self) -> None:
        self.file_path_input = QLineEdit()
        self.file_path_input.setPlaceholderText("Choose CSV or Flex XML file")
        self.browse_button = QPushButton("Browse")
        self.browse_button.clicked.connect(self._choose_file)

        self.import_type_combo = QComboBox()
        self.import_type_combo.addItem("Auto detect", ImportType.AUTO.value)
        self.import_type_combo.addItem("CSV", ImportType.CSV.value)
        self.import_type_combo.addItem("Flex XML", ImportType.FLEX.value)

        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 0)
        self.progress_bar.setVisible(False)

        self.error_label = QLabel("")
        self.error_label.setWordWrap(True)
        self.error_label.setStyleSheet("color: #b00020;")

        self.result_label = QLabel("")
        self.result_label.setWordWrap(True)

        self.import_button = QPushButton("Import")
        self.close_button = QPushButton("Close")
        self.import_button.clicked.connect(self.start_import)
        self.close_button.clicked.connect(self.reject)

    def _build_file_row(self) -> QWidget:
        file_row = QWidget()
        file_layout = QHBoxLayout(file_row)
        file_layout.setContentsMargins(0, 0, 0, 0)
        file_layout.setSpacing(8)
        file_layout.addWidget(self.file_path_input, 1)
        file_layout.addWidget(self.browse_button)
        return file_row

    def _build_form(self) -> QFormLayout:
        docs_label = QLabel(
            f'<a href="{IMPORT_DOCS_URL}">'
            "How to create CSV export in Interactive Brokers Portal"
            "</a>",
        )
        docs_label.setOpenExternalLinks(True)
        docs_label.setTextInteractionFlags(Qt.TextInteractionFlag.TextBrowserInteraction)

        form = QFormLayout()
        form.addRow("File", self._build_file_row())
        form.addRow("Type", self.import_type_combo)
        form.addRow("Documentation", docs_label)
        return form

    def _build_buttons_row(self) -> QHBoxLayout:
        buttons_row = QHBoxLayout()
        buttons_row.addStretch(1)
        buttons_row.addWidget(self.import_button)
        buttons_row.addWidget(self.close_button)
        return buttons_row

    def _choose_file(self) -> None:
        selected_file, _selected_filter = QFileDialog.getOpenFileName(
            self,
            "Select file to import",
            str(Path.home()),
            "Supported files (*.csv *.xml *.zip);;All files (*)",
        )
        if selected_file:
            self.file_path_input.setText(selected_file)

    def _selected_import_type(self) -> ImportType:
        return ImportType(self.import_type_combo.currentData())

    def _is_running(self) -> bool:
        return self.import_thread is not None and self.import_thread.isRunning()

    def _set_running_state(self, running: bool) -> None:
        self.progress_bar.setVisible(running)
        self.import_button.setEnabled(not running)
        self.close_button.setEnabled(not running)
        self.browse_button.setEnabled(not running)
        self.file_path_input.setEnabled(not running)
        self.import_type_combo.setEnabled(not running)

    @Slot()
    def start_import(self) -> None:
        if self._is_running():
            return

        file_path_text = self.file_path_input.text().strip()
        if not file_path_text:
            self.result_label.setText("")
            self.error_label.setText("Select a file first.")
            return

        file_path = Path(file_path_text).expanduser()
        if not file_path.exists():
            self.result_label.setText("")
            self.error_label.setText(f"File not found: {file_path}")
            return
        if file_path.is_dir():
            self.result_label.setText("")
            self.error_label.setText("Selected path is a directory, not a file.")
            return

        self.error_label.setText("")
        self.result_label.setText("")
        self._set_running_state(True)

        self.import_worker = ImportWorker(file_path, self._selected_import_type())
        self.import_thread = QThread(self)
        self.import_worker.moveToThread(self.import_thread)

        self.import_thread.started.connect(self.import_worker.run)
        self.import_worker.finished.connect(self.on_import_finished)
        self.import_worker.failed.connect(self.on_import_failed)

        self.import_worker.finished.connect(self.import_thread.quit)
        self.import_worker.finished.connect(self.import_worker.deleteLater)
        self.import_worker.failed.connect(self.import_thread.quit)
        self.import_worker.failed.connect(self.import_worker.deleteLater)

        self.import_thread.finished.connect(self.on_import_thread_finished)
        self.import_thread.finished.connect(self.import_thread.deleteLater)
        self.import_thread.start()

    @Slot(int, int, int)
    def on_import_finished(self, total: int, inserted: int, updated: int) -> None:
        if total == 0:
            self.result_label.setText("Import complete, no valid transactions found in file.")
        else:
            self.result_label.setText(
                "Import complete: parsed "
                f"{total} transaction(s) ({inserted} new, {updated} updated).",
            )
        self.error_label.setText("")
        self._set_running_state(False)

    @Slot(str)
    def on_import_failed(self, message: str) -> None:
        self.result_label.setText("")
        self.error_label.setText(f"Import failed: {message}")
        self._set_running_state(False)

    @Slot()
    def on_import_thread_finished(self) -> None:
        self.import_worker = None
        self.import_thread = None

    def reject(self) -> None:
        if self._is_running():
            self.error_label.setText("Import is running. Wait until it finishes.")
            return
        super().reject()

    @override
    def closeEvent(self, event: QCloseEvent) -> None:
        if self._is_running():
            self.error_label.setText("Import is running. Wait until it finishes.")
            event.ignore()
            return
        super().closeEvent(event)
