from __future__ import annotations

from decimal import Decimal, InvalidOperation

from PySide6.QtWidgets import (
    QCheckBox,
    QDialog,
    QDialogButtonBox,
    QFormLayout,
    QLabel,
    QLineEdit,
    QMessageBox,
    QVBoxLayout,
    QWidget,
)


class AssessmentDialog(QDialog):
    def __init__(
        self,
        declaration_id: str,
        initial_tax_due_rsd: Decimal | None,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self.setWindowTitle("Set Assessed Tax")
        self.setModal(True)
        self.resize(460, 220)

        self.tax_due_input = QLineEdit()
        self.tax_due_input.setPlaceholderText("0.00")
        if initial_tax_due_rsd is not None:
            self.tax_due_input.setText(f"{initial_tax_due_rsd:.2f}")

        self.mark_paid_checkbox = QCheckBox("Already paid")
        self.help_label = QLabel(
            f"Declaration: {declaration_id}. Enter official tax due from tax authority notice.",
        )
        self.help_label.setWordWrap(True)

        form = QFormLayout()
        form.addRow("Tax due (RSD)", self.tax_due_input)
        form.addRow("", self.mark_paid_checkbox)

        self.button_box = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Cancel | QDialogButtonBox.StandardButton.Ok,
        )
        self.button_box.accepted.connect(self.accept)
        self.button_box.rejected.connect(self.reject)

        layout = QVBoxLayout()
        layout.setSpacing(12)
        layout.addWidget(self.help_label)
        layout.addLayout(form)
        layout.addWidget(self.button_box)
        self.setLayout(layout)

        self.tax_due_rsd: Decimal | None = None
        self.mark_paid: bool = False

    def accept(self) -> None:
        raw_value = self.tax_due_input.text().strip()
        if not raw_value:
            QMessageBox.warning(self, "Invalid amount", "Enter tax due amount in RSD.")
            return
        try:
            tax_due_rsd = Decimal(raw_value)
        except (InvalidOperation, TypeError, ValueError):
            QMessageBox.warning(self, "Invalid amount", "Tax due must be a decimal number.")
            return
        if tax_due_rsd < Decimal("0"):
            QMessageBox.warning(self, "Invalid amount", "Tax due must be non-negative.")
            return

        self.tax_due_rsd = tax_due_rsd.quantize(Decimal("0.01"))
        self.mark_paid = self.mark_paid_checkbox.isChecked()
        super().accept()
