from __future__ import annotations

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QLineEdit,
    QVBoxLayout,
    QWidget,
)

from ibkr_porez.models import UserConfig


class ConfigDialog(QDialog):
    def __init__(self, config: UserConfig, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle("Config")
        self.setModal(True)
        self._base_config = config

        form = QFormLayout()

        self.ibkr_token = QLineEdit(config.ibkr_token)
        self.ibkr_query_id = QLineEdit(config.ibkr_query_id)
        self.data_dir = QLineEdit(config.data_dir or "")
        self.output_folder = QLineEdit(config.output_folder or "")

        form.addRow("IBKR Flex Token", self.ibkr_token)
        form.addRow("IBKR Query ID", self.ibkr_query_id)
        form.addRow("Data Directory", self.data_dir)
        form.addRow("Output Folder", self.output_folder)

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

    def get_config(self) -> UserConfig:
        data = self._base_config.model_dump()
        data["ibkr_token"] = self.ibkr_token.text().strip()
        data["ibkr_query_id"] = self.ibkr_query_id.text().strip()
        data["data_dir"] = self.data_dir.text().strip() or None
        data["output_folder"] = self.output_folder.text().strip() or None
        return UserConfig(**data)
