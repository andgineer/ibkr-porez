from __future__ import annotations

from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QPlainTextEdit,
    QVBoxLayout,
    QWidget,
)

from ibkr_porez.operation_show_declaration import render_declaration_details_text


class DeclarationDetailsDialog(QDialog):
    def __init__(self, declaration_id: str, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self.setWindowTitle(f"Declaration {declaration_id}")
        self.resize(920, 680)

        self.details_view = QPlainTextEdit()
        self.details_view.setReadOnly(True)
        self.details_view.setLineWrapMode(QPlainTextEdit.LineWrapMode.NoWrap)
        self.details_view.setPlainText(render_declaration_details_text(declaration_id))

        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Close)
        buttons.rejected.connect(self.reject)
        buttons.accepted.connect(self.accept)

        layout = QVBoxLayout()
        layout.setSpacing(10)
        layout.addWidget(self.details_view)
        layout.addWidget(buttons)
        self.setLayout(layout)
