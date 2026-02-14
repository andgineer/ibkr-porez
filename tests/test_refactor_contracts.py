from __future__ import annotations

import os
import sys
from datetime import date, datetime
from io import StringIO
from pathlib import Path

import allure
import pytest
from click.testing import CliRunner
from rich.console import Console

import ibkr_porez.declaration_manager as declaration_manager_module
import ibkr_porez.gui.import_dialog as import_dialog_module
import ibkr_porez.operation_list as operation_list_module
import ibkr_porez.operation_report as operation_report_module
import ibkr_porez.operation_sync as operation_sync_module
from ibkr_porez.declaration_manager import DeclarationManager
from ibkr_porez.main import ibkr_porez
from ibkr_porez.models import Declaration, DeclarationStatus, DeclarationType, UserConfig
from ibkr_porez.operation_import import ImportType
from ibkr_porez.operation_list import ListDeclarations
from ibkr_porez.operation_report import process_gains_report
from ibkr_porez.operation_sync import SyncOperation

if sys.platform == "linux":
    from PySide6.QtWidgets import QApplication


class _FakeListStorage:
    def __init__(self, declarations: list[Declaration]) -> None:
        self._declarations = [declaration.model_copy(deep=True) for declaration in declarations]

    def get_declarations(
        self,
        status: DeclarationStatus | None = None,
        declaration_type: DeclarationType | None = None,  # noqa: ARG002
    ) -> list[Declaration]:
        declarations = [declaration.model_copy(deep=True) for declaration in self._declarations]
        if status is None:
            return declarations
        return [declaration for declaration in declarations if declaration.status == status]


def _make_declaration(
    declaration_id: str,
    status: DeclarationStatus,
    created_at: datetime,
) -> Declaration:
    return Declaration(
        declaration_id=declaration_id,
        type=DeclarationType.PPDG3R,
        status=status,
        period_start=date(2026, 1, 1),
        period_end=date(2026, 1, 31),
        created_at=created_at,
        xml_content=f"<xml>{declaration_id}</xml>",
    )


@allure.epic("Contracts")
@allure.feature("Refactor Safety")
def test_cli_list_ids_only_matches_controller_output_for_same_storage(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    declarations = [
        _make_declaration("draft-1", DeclarationStatus.DRAFT, datetime(2026, 2, 5, 9, 0, 0)),
        _make_declaration(
            "submitted-1",
            DeclarationStatus.SUBMITTED,
            datetime(2026, 2, 4, 9, 0, 0),
        ),
        _make_declaration("pending-1", DeclarationStatus.PENDING, datetime(2026, 2, 3, 9, 0, 0)),
        _make_declaration(
            "finalized-1",
            DeclarationStatus.FINALIZED,
            datetime(2026, 2, 2, 9, 0, 0),
        ),
    ]
    fake_storage = _FakeListStorage(declarations)
    monkeypatch.setattr(operation_list_module, "Storage", lambda: fake_storage)

    expected_ids = ListDeclarations().generate(ids_only=True)
    runner = CliRunner()
    result = runner.invoke(ibkr_porez, ["list", "--ids-only"])

    assert result.exit_code == 0
    actual_ids = [line.strip() for line in result.output.splitlines() if line.strip()]
    assert actual_ids == expected_ids


@allure.epic("Contracts")
@allure.feature("Refactor Safety")
def test_output_folder_contract_sync_report_and_export(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    output_folder = tmp_path / "contract-output"
    config = UserConfig(
        full_name="Contract User",
        address="Contract Address",
        ibkr_token="token",
        ibkr_query_id="query",
        output_folder=str(output_folder),
    )

    monkeypatch.setattr(operation_sync_module.config_manager, "load_config", lambda: config)
    monkeypatch.setattr(operation_report_module.config_manager, "load_config", lambda: config)
    monkeypatch.setattr(declaration_manager_module.config_manager, "load_config", lambda: config)

    sync_output_folder = SyncOperation(config).get_output_folder()
    assert sync_output_folder == output_folder

    class FakeGainsReportGenerator:
        @staticmethod
        def generate(
            start_date: date,  # noqa: ARG004
            end_date: date,  # noqa: ARG004
        ) -> list[tuple[str, str, list]]:
            return [("contract-ppdg.xml", "<xml>report</xml>", [])]

    monkeypatch.setattr(operation_report_module, "GainsReportGenerator", FakeGainsReportGenerator)

    process_gains_report(
        start_date=date(2026, 1, 1),
        end_date=date(2026, 1, 31),
        filename=None,
        console=Console(file=StringIO(), force_terminal=False, color_system=None),
        output_dir=None,
    )
    assert (output_folder / "contract-ppdg.xml").exists()

    manager = DeclarationManager()
    declaration = _make_declaration(
        declaration_id="export-contract",
        status=DeclarationStatus.DRAFT,
        created_at=datetime(2026, 2, 1, 9, 0, 0),
    )
    declaration.file_path = str(manager.storage.declarations_dir / "export-contract.xml")
    manager.storage.save_declaration(declaration)

    xml_path, _attached_paths = manager.export("export-contract", output_dir=None)
    assert xml_path.parent == output_folder


@allure.epic("Contracts")
@allure.feature("Refactor Safety")
@pytest.mark.skipif(
    sys.platform != "linux",
    reason="Qt UI tests run in CI only on Linux",
)
def test_import_type_contract_cli_and_gui_match_enum() -> None:
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    app = QApplication.instance()
    if app is None:
        app = QApplication(["pytest"])

    expected = {import_type.value for import_type in ImportType}

    import_command = ibkr_porez.commands["import"]
    import_option = next(
        parameter
        for parameter in import_command.params
        if getattr(parameter, "name", None) == "import_type"
    )
    assert set(import_option.type.choices) == expected

    dialog = import_dialog_module.ImportDialog()
    try:
        gui_values = {
            str(dialog.import_type_combo.itemData(index))
            for index in range(dialog.import_type_combo.count())
        }
        assert gui_values == expected
    finally:
        dialog.close()
