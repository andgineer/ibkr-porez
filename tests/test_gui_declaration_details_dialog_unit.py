from __future__ import annotations

import allure
import pytest
from PySide6.QtWidgets import QApplication

import ibkr_porez.gui.declaration_details_dialog as details_dialog_module
from ibkr_porez.gui.declaration_details_dialog import DeclarationDetailsDialog


@pytest.fixture(scope="module")
def qapp() -> QApplication:
    app = QApplication.instance()
    if app is None:
        app = QApplication(["pytest"])
    return app


@allure.epic("GUI")
@allure.feature("Declaration Details")
class TestDeclarationDetailsDialogUnit:
    def test_dialog_renders_text_for_selected_declaration(
        self,
        qapp: QApplication,  # noqa: ARG002
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        monkeypatch.setattr(
            details_dialog_module,
            "render_declaration_details_text",
            lambda declaration_id: f"Details for {declaration_id}",
        )

        dialog = DeclarationDetailsDialog("decl-123")
        try:
            assert dialog.windowTitle() == "Declaration decl-123"
            assert dialog.details_view.toPlainText() == "Details for decl-123"
            assert dialog.details_view.isReadOnly()
            assert dialog.details_view.lineWrapMode() == dialog.details_view.LineWrapMode.NoWrap
        finally:
            dialog.close()
