from __future__ import annotations

import allure
import pytest

import ibkr_porez.operation_show_declaration as show_module


@allure.epic("CLI")
@allure.feature("Show")
class TestOperationShowDeclarationUnit:
    def test_render_declaration_details_text_uses_show_declaration_output(
        self,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        def fake_show_declaration(declaration_id: str, console) -> None:
            assert declaration_id == "decl-42"
            console.print("Declaration details text")

        monkeypatch.setattr(show_module, "show_declaration", fake_show_declaration)

        assert show_module.render_declaration_details_text("decl-42") == "Declaration details text"
