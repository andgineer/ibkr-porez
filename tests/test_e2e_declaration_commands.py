"""E2E tests for declaration management commands (list, submit, pay, export, revert, attach)."""

import pytest
from click.testing import CliRunner
from datetime import date, datetime
from pathlib import Path
from unittest.mock import patch

from ibkr_porez.main import ibkr_porez
from ibkr_porez.storage import Storage
from ibkr_porez.models import (
    Declaration,
    DeclarationStatus,
    DeclarationType,
    UserConfig,
)


@pytest.fixture
def mock_user_data_dir(tmp_path):
    mock_config = UserConfig(full_name="Test", address="Test", data_dir=None, output_folder=None)
    with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
        with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
            with patch(
                "ibkr_porez.declaration_manager.config_manager.load_config",
                return_value=mock_config,
            ):
                s = Storage()
                s._ensure_dirs()
                yield tmp_path


@pytest.fixture
def runner():
    return CliRunner()


@pytest.fixture
def setup_declarations(mock_user_data_dir):
    """Create test declarations."""
    mock_config = UserConfig(full_name="Test", address="Test", data_dir=None, output_folder=None)
    with patch("ibkr_porez.storage.user_data_dir", lambda app: str(mock_user_data_dir)):
        with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
            with patch(
                "ibkr_porez.declaration_manager.config_manager.load_config",
                return_value=mock_config,
            ):
                s = Storage()

                decl1 = Declaration(
                    declaration_id="1",
                    type=DeclarationType.PPDG3R,
                    status=DeclarationStatus.DRAFT,
                    period_start=date(2024, 1, 1),
                    period_end=date(2024, 6, 30),
                    created_at=datetime(2024, 1, 1, 10, 0),
                    file_path=str(s.declarations_dir / "1-ppdg3r-2024-H1.xml"),
                    xml_content="<xml>test1</xml>",
                    report_data=[],
                )

                decl2 = Declaration(
                    declaration_id="2",
                    type=DeclarationType.PPO,
                    status=DeclarationStatus.DRAFT,
                    period_start=date(2024, 1, 15),
                    period_end=date(2024, 1, 15),
                    created_at=datetime(2024, 1, 15, 10, 0),
                    file_path=str(s.declarations_dir / "2-ppopo-2024-01-15_dividend.xml"),
                    xml_content="<xml>test2</xml>",
                    report_data=[],
                )

                decl3 = Declaration(
                    declaration_id="3",
                    type=DeclarationType.PPDG3R,
                    status=DeclarationStatus.SUBMITTED,
                    period_start=date(2023, 7, 1),
                    period_end=date(2023, 12, 31),
                    created_at=datetime(2023, 12, 31, 10, 0),
                    submitted_at=datetime(2024, 1, 5, 10, 0),
                    file_path=str(s.declarations_dir / "3-ppdg3r-2023-H2.xml"),
                    xml_content="<xml>test3</xml>",
                    report_data=[],
                )

                s.save_declaration(decl1)
                s.save_declaration(decl2)
                s.save_declaration(decl3)

                # Create XML files
                Path(decl1.file_path).parent.mkdir(parents=True, exist_ok=True)
                Path(decl1.file_path).write_text(decl1.xml_content)
                Path(decl2.file_path).parent.mkdir(parents=True, exist_ok=True)
                Path(decl2.file_path).write_text(decl2.xml_content)
                Path(decl3.file_path).parent.mkdir(parents=True, exist_ok=True)
                Path(decl3.file_path).write_text(decl3.xml_content)

                yield s


class TestE2EList:
    def test_list_default_shows_active(self, runner, setup_declarations, mock_user_data_dir):
        """Test that list command shows active (DRAFT + SUBMITTED) by default."""
        mock_config = UserConfig(
            full_name="Test", address="Test", data_dir=None, output_folder=None
        )
        with patch("ibkr_porez.storage.user_data_dir", lambda app: str(mock_user_data_dir)):
            with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
                with patch("ibkr_porez.operation_list.Storage", return_value=setup_declarations):
                    result = runner.invoke(ibkr_porez, ["list"])
                    assert result.exit_code == 0
                    assert "│ 1  │" in result.output
                    assert "│ 2  │" in result.output
                    assert "│ 3  │" in result.output  # Submitted is active by default

    def test_list_all_shows_all(self, runner, setup_declarations, mock_user_data_dir):
        """Test that list --all shows all declarations."""
        mock_config = UserConfig(
            full_name="Test", address="Test", data_dir=None, output_folder=None
        )
        with patch("ibkr_porez.storage.user_data_dir", lambda app: str(mock_user_data_dir)):
            with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
                with patch("ibkr_porez.operation_list.Storage", return_value=setup_declarations):
                    result = runner.invoke(ibkr_porez, ["list", "--all"])
                    assert result.exit_code == 0
                    assert "│ 1  │" in result.output
                    assert "│ 2  │" in result.output
                    assert "│ 3  │" in result.output

    def test_list_filter_by_status(self, runner, setup_declarations, mock_user_data_dir):
        """Test that list --status filters correctly."""
        mock_config = UserConfig(
            full_name="Test", address="Test", data_dir=None, output_folder=None
        )
        with patch("ibkr_porez.storage.user_data_dir", lambda app: str(mock_user_data_dir)):
            with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
                with patch("ibkr_porez.operation_list.Storage", return_value=setup_declarations):
                    result = runner.invoke(ibkr_porez, ["list", "--status", "submitted"])
                    assert result.exit_code == 0
                    assert "│ 1  │" not in result.output
                    assert "│ 2  │" not in result.output
                    assert "│ 3  │" in result.output

    def test_list_empty(self, runner, mock_user_data_dir):
        """Test that list shows empty table when no declarations exist."""
        result = runner.invoke(ibkr_porez, ["list"])
        assert result.exit_code == 0
        assert "Declarations" in result.output

    def test_list_ids_only(self, runner, setup_declarations, mock_user_data_dir):
        """Test that list --ids-only outputs active IDs by default."""
        mock_config = UserConfig(
            full_name="Test", address="Test", data_dir=None, output_folder=None
        )
        with patch("ibkr_porez.storage.user_data_dir", lambda app: str(mock_user_data_dir)):
            with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
                with patch("ibkr_porez.operation_list.Storage", return_value=setup_declarations):
                    result = runner.invoke(ibkr_porez, ["list", "--ids-only"])
                    assert result.exit_code == 0
                    assert "1" in result.output
                    assert "2" in result.output
                    assert "3" in result.output  # Submitted is active by default
                    # Should be one ID per line
                    lines = [
                        line.strip() for line in result.output.strip().split("\n") if line.strip()
                    ]
                    assert "1" in lines
                    assert "2" in lines
                    assert "3" in lines

    def test_list_ids_only_short_flag(self, runner, setup_declarations, mock_user_data_dir):
        """Test that list -1 outputs only IDs (short flag)."""
        mock_config = UserConfig(
            full_name="Test", address="Test", data_dir=None, output_folder=None
        )
        with patch("ibkr_porez.storage.user_data_dir", lambda app: str(mock_user_data_dir)):
            with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
                with patch("ibkr_porez.operation_list.Storage", return_value=setup_declarations):
                    result = runner.invoke(ibkr_porez, ["list", "-1"])
                    assert result.exit_code == 0
                    assert "1" in result.output
                    assert "2" in result.output
                    assert "3" in result.output
                    # Should be one ID per line
                    lines = [
                        line.strip() for line in result.output.strip().split("\n") if line.strip()
                    ]
                    assert "1" in lines
                    assert "2" in lines
                    assert "3" in lines


class TestE2ESubmit:
    def test_submit_single_declaration(self, runner, setup_declarations):
        """Test submitting a single declaration."""
        result = runner.invoke(ibkr_porez, ["submit", "1"])
        assert result.exit_code == 0
        assert "Submitted: 1" in result.output

        decl = setup_declarations.get_declaration("1")
        assert decl.status == DeclarationStatus.SUBMITTED
        assert decl.submitted_at is not None

    def test_submit_nonexistent(self, runner, setup_declarations):
        """Test submitting non-existent declaration."""
        result = runner.invoke(ibkr_porez, ["submit", "999"])
        assert result.exit_code != 0
        assert "not found" in result.output.lower()

    def test_submit_already_submitted(self, runner, setup_declarations):
        """Test submitting already submitted declaration."""
        result = runner.invoke(ibkr_porez, ["submit", "3"])
        assert result.exit_code != 0
        assert "not in DRAFT status" in result.output


class TestE2EPay:
    def test_pay_single_declaration(self, runner, setup_declarations):
        """Test paying a single declaration."""
        result = runner.invoke(ibkr_porez, ["pay", "1"])
        assert result.exit_code == 0
        assert "Paid: 1" in result.output

        decl = setup_declarations.get_declaration("1")
        assert decl.status == DeclarationStatus.PAID
        assert decl.paid_at is not None

    def test_pay_submitted_declaration(self, runner, setup_declarations):
        """Test paying a submitted declaration."""
        result = runner.invoke(ibkr_porez, ["pay", "3"])
        assert result.exit_code == 0
        assert "Paid: 3" in result.output

        decl = setup_declarations.get_declaration("3")
        assert decl.status == DeclarationStatus.PAID


class TestE2EExport:
    def test_export_declaration(self, runner, setup_declarations, tmp_path):
        """Test exporting a declaration."""
        output_dir = tmp_path / "export"
        result = runner.invoke(ibkr_porez, ["export", "1", "-o", str(output_dir)])
        assert result.exit_code == 0
        assert "Exported XML" in result.output

        xml_file = output_dir / "1-ppdg3r-2024-H1.xml"
        assert xml_file.exists()
        assert xml_file.read_text() == "<xml>test1</xml>"

    def test_export_with_attached_files(self, runner, setup_declarations, tmp_path):
        """Test exporting declaration with attached files."""
        decl = setup_declarations.get_declaration("1")

        # Attach a file
        test_file = tmp_path / "test_attach.txt"
        test_file.write_text("test content")

        # Manually attach file (simulating attach command)
        attachments_dir = setup_declarations.declarations_dir / "1" / "attachments"
        attachments_dir.mkdir(parents=True, exist_ok=True)
        dest_file = attachments_dir / "test_attach.txt"
        dest_file.write_text("test content")

        decl.attached_files["test_attach.txt"] = str(
            dest_file.relative_to(setup_declarations.declarations_dir)
        )
        setup_declarations.save_declaration(decl)

        # Export
        output_dir = tmp_path / "export"
        result = runner.invoke(ibkr_porez, ["export", "1", "-o", str(output_dir)])
        assert result.exit_code == 0
        assert "Exported XML" in result.output
        assert "1 attached file" in result.output

        attached_file = output_dir / "test_attach.txt"
        assert attached_file.exists()
        assert attached_file.read_text() == "test content"

    def test_export_nonexistent(self, runner, setup_declarations):
        """Test exporting non-existent declaration."""
        result = runner.invoke(ibkr_porez, ["export", "999"])
        assert result.exit_code != 0
        assert "not found" in result.output.lower()


class TestE2ERevert:
    def test_revert_paid_to_draft(self, runner, setup_declarations):
        """Test reverting paid declaration to draft."""
        # First pay it
        runner.invoke(ibkr_porez, ["pay", "1"])

        # Then revert
        result = runner.invoke(ibkr_porez, ["revert", "1", "--to", "draft"])
        assert result.exit_code == 0
        assert "Reverted 1 to draft" in result.output

        decl = setup_declarations.get_declaration("1")
        assert decl.status == DeclarationStatus.DRAFT
        assert decl.paid_at is None

    def test_revert_submitted_to_draft(self, runner, setup_declarations):
        """Test reverting submitted declaration to draft."""
        result = runner.invoke(ibkr_porez, ["revert", "3", "--to", "draft"])
        assert result.exit_code == 0

        decl = setup_declarations.get_declaration("3")
        assert decl.status == DeclarationStatus.DRAFT
        assert decl.submitted_at is None

    def test_revert_draft_fails(self, runner, setup_declarations):
        """Test that reverting draft declaration fails."""
        result = runner.invoke(ibkr_porez, ["revert", "1", "--to", "draft"])
        assert result.exit_code != 0
        assert "Cannot revert" in result.output

    def test_list_pipe_to_submit(self, runner, setup_declarations):
        """Test Unix-style pipe: list --status draft -1 | xargs -I {} submit {}."""
        # Get IDs from list filtered to draft status
        list_result = runner.invoke(ibkr_porez, ["list", "--status", "draft", "-1"])
        assert list_result.exit_code == 0
        ids = [line.strip() for line in list_result.output.strip().split("\n") if line.strip()]
        assert len(ids) >= 2

        # Submit each ID individually (simulating xargs)
        for decl_id in ids:
            submit_result = runner.invoke(ibkr_porez, ["submit", decl_id])
            assert submit_result.exit_code == 0

        for decl_id in ids:
            decl = setup_declarations.get_declaration(decl_id)
            assert decl.status == DeclarationStatus.SUBMITTED


class TestE2EAttach:
    def test_attach_file(self, runner, setup_declarations, tmp_path):
        """Test attaching a file to declaration."""
        test_file = tmp_path / "test_file.txt"
        test_file.write_text("test content")

        result = runner.invoke(ibkr_porez, ["attach", "1", str(test_file)])
        assert result.exit_code == 0
        assert "Attached file 'test_file.txt'" in result.output

        decl = setup_declarations.get_declaration("1")
        assert "test_file.txt" in decl.attached_files

        # Check file was copied
        attachments_dir = setup_declarations.declarations_dir / "1" / "attachments"
        assert (attachments_dir / "test_file.txt").exists()

    def test_detach_file(self, runner, setup_declarations, tmp_path):
        """Test detaching a file from declaration."""
        # First attach
        test_file = tmp_path / "test_file.txt"
        test_file.write_text("test content")
        runner.invoke(ibkr_porez, ["attach", "1", str(test_file)])

        # Then detach
        result = runner.invoke(ibkr_porez, ["attach", "1", "test_file.txt", "--delete"])
        assert result.exit_code == 0
        assert "Removed file 'test_file.txt'" in result.output

        decl = setup_declarations.get_declaration("1")
        assert "test_file.txt" not in decl.attached_files

    def test_attach_nonexistent_file(self, runner, setup_declarations):
        """Test attaching non-existent file."""
        result = runner.invoke(ibkr_porez, ["attach", "1", "/nonexistent/file.txt"])
        assert result.exit_code != 0
        assert "File not found" in result.output

    def test_detach_nonexistent_file(self, runner, setup_declarations):
        """Test detaching non-existent file."""
        result = runner.invoke(ibkr_porez, ["attach", "1", "nonexistent.txt", "--delete"])
        assert result.exit_code != 0
        assert "not found" in result.output.lower()
