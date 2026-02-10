from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

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

        self.data_dir.setPlaceholderText("Default app data directory if empty")
        self.output_folder.setPlaceholderText("Downloads if empty")

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
            f'<a href="{FLEX_DOCS_URL}">Open setup docs for Flex Web Service</a>',
        )
        docs_label.setOpenExternalLinks(True)
        docs_label.setTextInteractionFlags(Qt.TextInteractionFlag.TextBrowserInteraction)
        form.addRow("Documentation", docs_label)
        return group

    def _build_app_group(self) -> QGroupBox:
        group = QGroupBox("App Settings")
        form = QFormLayout(group)
        form.addRow("Data Directory", self._build_path_row(self.data_dir, self._choose_data_dir))
        form.addRow(
            "Output Folder",
            self._build_path_row(self.output_folder, self._choose_output_folder),
        )
        return group

    def _build_path_row(self, field: QLineEdit, browse_handler) -> QWidget:
        wrapper = QWidget()
        row = QHBoxLayout(wrapper)
        row.setContentsMargins(0, 0, 0, 0)
        row.setSpacing(8)

        browse_button = QPushButton("Browse")
        browse_button.clicked.connect(browse_handler)
        row.addWidget(field, 1)
        row.addWidget(browse_button)
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
