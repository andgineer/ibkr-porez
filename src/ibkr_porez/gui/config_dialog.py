from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMessageBox,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from ibkr_porez.config import config_manager, get_default_data_dir_path, get_default_output_dir_path
from ibkr_porez.models import UserConfig

FLEX_DOCS_URL = "https://andgineer.github.io/ibkr-porez/ibkr/#flex-web-service"


class ConfigDialog(QDialog):
    def __init__(self, config: UserConfig, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Config")
        self.setModal(True)
        self.resize(760, 560)
        self._base_config = config

        self.ibkr_token = QLineEdit(config.ibkr_token)
        self.ibkr_query_id = QLineEdit(config.ibkr_query_id)
        self.personal_id = QLineEdit(config.personal_id)
        self.full_name = QLineEdit(config.full_name)
        self.address = QLineEdit(config.address)
        self.city_code = QLineEdit(config.city_code)
        self.phone = QLineEdit(config.phone)
        self.email = QLineEdit(config.email)
        self.data_dir = QLineEdit(config.data_dir or "")
        self.output_folder = QLineEdit(config.output_folder or "")
        self.app_files_dir = config_manager.config_path.parent
        self.app_files_info = QLabel(str(self.app_files_dir))

        self.data_dir.setPlaceholderText("Default app data directory if empty")
        self.output_folder.setPlaceholderText("Downloads if empty")
        self.app_files_info.setWordWrap(True)
        self.app_files_info.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)

        layout = QVBoxLayout()
        layout.setSpacing(12)
        layout.addWidget(self._build_taxpayer_group())
        layout.addWidget(self._build_ibkr_group())
        layout.addWidget(self._build_app_group())

        buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Save | QDialogButtonBox.StandardButton.Cancel,
            Qt.Orientation.Horizontal,
            self,
        )
        buttons.accepted.connect(self.accept)
        buttons.rejected.connect(self.reject)
        layout.addWidget(buttons)

        self.setLayout(layout)

    def _build_taxpayer_group(self) -> QGroupBox:
        group = QGroupBox("Personal Taxpayer Data")
        form = QFormLayout(group)
        form.addRow("Personal ID (JMBG / EBS)", self.personal_id)
        form.addRow("Full Name", self.full_name)
        form.addRow("Address", self.address)
        form.addRow("City Code", self.city_code)
        form.addRow("Phone", self.phone)
        form.addRow("Email", self.email)
        return group

    def _build_ibkr_group(self) -> QGroupBox:
        group = QGroupBox("IBKR Flex Parameters")
        form = QFormLayout(group)
        form.addRow("Flex Token", self.ibkr_token)
        form.addRow("Flex Query ID", self.ibkr_query_id)

        docs_label = QLabel(
            f'<a href="{FLEX_DOCS_URL}">How to get Flex Token and Flex Query ID in IBKR</a>',
        )
        docs_label.setOpenExternalLinks(True)
        docs_label.setTextInteractionFlags(Qt.TextInteractionFlag.TextBrowserInteraction)
        form.addRow(docs_label)
        return group

    def _build_app_group(self) -> QGroupBox:
        group = QGroupBox("App Settings")
        form = QFormLayout(group)
        form.addRow(
            "Data Directory",
            self._build_path_row(
                self.data_dir,
                self._choose_data_dir,
                self._open_data_dir,
            ),
        )
        form.addRow(
            "Output Folder",
            self._build_path_row(
                self.output_folder,
                self._choose_output_folder,
                self._open_output_folder,
            ),
        )
        form.addRow(
            "Config and Logs",
            self._build_info_row(
                self.app_files_info,
                self._open_app_files_dir,
            ),
        )
        return group

    def _build_path_row(self, field: QLineEdit, browse_handler, open_handler) -> QWidget:
        wrapper = QWidget()
        row = QHBoxLayout(wrapper)
        row.setContentsMargins(0, 0, 0, 0)
        row.setSpacing(8)

        browse_button = QPushButton("Choose...")
        browse_button.setToolTip("Select folder path")
        open_button = QPushButton("Open Folder")
        open_button.setToolTip("Open folder in file manager")
        browse_button.clicked.connect(browse_handler)
        open_button.clicked.connect(open_handler)
        row.addWidget(field, 1)
        row.addWidget(browse_button)
        row.addWidget(open_button)
        return wrapper

    def _build_info_row(self, field: QLabel, open_handler) -> QWidget:
        wrapper = QWidget()
        row = QHBoxLayout(wrapper)
        row.setContentsMargins(0, 0, 0, 0)
        row.setSpacing(8)

        open_button = QPushButton("Open Folder")
        open_button.setToolTip("Open folder in file manager")
        open_button.clicked.connect(open_handler)
        row.addWidget(field, 1)
        row.addWidget(open_button)
        return wrapper

    def _choose_data_dir(self) -> None:
        self._choose_directory(self.data_dir, "Select Data Directory")

    def _choose_output_folder(self) -> None:
        self._choose_directory(self.output_folder, "Select Output Folder")

    def _choose_directory(self, field: QLineEdit, title: str) -> None:
        current_value = field.text().strip()
        start_dir = current_value or str(Path.home())
        chosen = QFileDialog.getExistingDirectory(self, title, start_dir)
        if chosen:
            field.setText(chosen)

    def _open_data_dir(self) -> None:
        self._open_directory(self.data_dir.text().strip(), get_default_data_dir_path())

    def _open_output_folder(self) -> None:
        self._open_directory(self.output_folder.text().strip(), get_default_output_dir_path())

    def _open_app_files_dir(self) -> None:
        self._open_directory(str(self.app_files_dir), self.app_files_dir)

    def _open_directory(self, value: str, fallback: Path) -> None:
        directory = Path(value).expanduser() if value else fallback
        try:
            directory.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            QMessageBox.warning(
                self,
                "Open folder failed",
                f"Cannot create folder: {directory}\n\n{error}",
            )
            return

        if not QDesktopServices.openUrl(QUrl.fromLocalFile(str(directory))):
            QMessageBox.warning(
                self,
                "Open folder failed",
                f"Cannot open folder: {directory}",
            )

    def get_config(self) -> UserConfig:
        data = self._base_config.model_dump()
        data["ibkr_token"] = self.ibkr_token.text().strip()
        data["ibkr_query_id"] = self.ibkr_query_id.text().strip()
        data["personal_id"] = self.personal_id.text().strip()
        data["full_name"] = self.full_name.text().strip()
        data["address"] = self.address.text().strip()
        data["city_code"] = self.city_code.text().strip() or "223"
        data["phone"] = self.phone.text().strip() or "0600000000"
        data["email"] = self.email.text().strip() or "email@example.com"
        data["data_dir"] = self.data_dir.text().strip() or None
        data["output_folder"] = self.output_folder.text().strip() or None
        return UserConfig(**data)
