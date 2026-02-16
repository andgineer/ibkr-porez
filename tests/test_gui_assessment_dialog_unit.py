from __future__ import annotations

from decimal import Decimal

import allure
import pytest
from PySide6.QtWidgets import QApplication

from ibkr_porez.gui.assessment_dialog import AssessmentDialog


@pytest.fixture(scope="module")
def qapp() -> QApplication:
    app = QApplication.instance()
    if app is None:
        app = QApplication(["pytest"])
    return app


@allure.epic("GUI")
@allure.feature("Assessment Dialog")
class TestAssessmentDialogUnit:
    def test_accept_valid_amount_sets_tax_and_paid_flag(
        self,
        qapp: QApplication,  # noqa: ARG002
    ) -> None:
        dialog = AssessmentDialog(
            declaration_id="decl-1",
            initial_tax_due_rsd=Decimal("10.00"),
        )
        try:
            dialog.tax_due_input.setText("12.345")
            dialog.mark_paid_checkbox.setChecked(True)

            dialog.accept()

            assert dialog.tax_due_rsd == Decimal("12.34")
            assert dialog.mark_paid is True
            assert dialog.result() == int(dialog.DialogCode.Accepted)
        finally:
            dialog.close()

    @pytest.mark.parametrize(
        ("raw_value", "expected_message"),
        [
            ("", "Enter tax due amount in RSD."),
            ("abc", "Tax due must be a decimal number."),
            ("-1", "Tax due must be non-negative."),
        ],
    )
    def test_accept_invalid_amount_shows_warning_and_keeps_dialog_open(
        self,
        qapp: QApplication,  # noqa: ARG002
        monkeypatch: pytest.MonkeyPatch,
        raw_value: str,
        expected_message: str,
    ) -> None:
        warnings: list[tuple[str, str]] = []
        dialog = AssessmentDialog(
            declaration_id="decl-1",
            initial_tax_due_rsd=None,
        )
        monkeypatch.setattr(
            "ibkr_porez.gui.assessment_dialog.QMessageBox.warning",
            lambda _parent, title, text: warnings.append((title, text)),
        )
        try:
            dialog.tax_due_input.setText(raw_value)

            dialog.accept()

            assert warnings == [("Invalid amount", expected_message)]
            assert dialog.tax_due_rsd is None
            assert dialog.result() != int(dialog.DialogCode.Accepted)
        finally:
            dialog.close()
