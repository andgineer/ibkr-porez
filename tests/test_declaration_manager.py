"""Unit tests for DeclarationManager."""

import allure
import pytest
from datetime import date, datetime
from pathlib import Path
from unittest.mock import patch

from ibkr_porez.declaration_manager import DeclarationManager
from ibkr_porez.models import (
    Declaration,
    DeclarationStatus,
    DeclarationType,
    UserConfig,
)
from ibkr_porez.storage import Storage


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
def manager(mock_user_data_dir):
    return DeclarationManager()


@pytest.fixture
def sample_declaration(mock_user_data_dir):
    """Create a sample declaration for testing."""
    s = Storage()
    decl = Declaration(
        declaration_id="test-1",
        type=DeclarationType.PPDG3R,
        status=DeclarationStatus.DRAFT,
        period_start=date(2024, 1, 1),
        period_end=date(2024, 6, 30),
        created_at=datetime(2024, 1, 1, 10, 0),
        file_path=str(s.declarations_dir / "test-1-ppdg3r-2024-H1.xml"),
        xml_content="<xml>test content</xml>",
        report_data=[],
    )
    s.save_declaration(decl)

    # Create XML file
    Path(decl.file_path).parent.mkdir(parents=True, exist_ok=True)
    Path(decl.file_path).write_text(decl.xml_content)

    return decl


class TestDeclarationManagerSubmit:
    def test_submit_single_declaration(self, manager, sample_declaration):
        """Test submitting a single declaration."""
        ids = manager.submit(["test-1"])
        assert ids == ["test-1"]

        s = Storage()
        decl = s.get_declaration("test-1")
        assert decl.status == DeclarationStatus.SUBMITTED
        assert decl.submitted_at is not None

    def test_submit_multiple_declarations(self, manager, mock_user_data_dir):
        """Test submitting multiple declarations."""
        s = Storage()

        # Create multiple draft declarations
        for i in range(3):
            decl = Declaration(
                declaration_id=f"draft-{i}",
                type=DeclarationType.PPDG3R,
                status=DeclarationStatus.DRAFT,
                period_start=date(2024, 1, 1),
                period_end=date(2024, 6, 30),
                created_at=datetime(2024, 1, 1, 10, 0),
                file_path=str(s.declarations_dir / f"draft-{i}.xml"),
                xml_content="<xml>test</xml>",
                report_data=[],
            )
            s.save_declaration(decl)

        ids = manager.submit(["draft-0", "draft-1", "draft-2"])
        assert len(ids) == 3
        assert all(id in ids for id in ["draft-0", "draft-1", "draft-2"])

        for id in ids:
            decl = s.get_declaration(id)
            assert decl.status == DeclarationStatus.SUBMITTED

    def test_submit_nonexistent(self, manager):
        """Test submitting non-existent declaration."""
        with pytest.raises(ValueError, match="not found"):
            manager.submit(["nonexistent"])

    def test_submit_already_submitted(self, manager, sample_declaration):
        """Test submitting already submitted declaration."""
        manager.submit(["test-1"])

        with pytest.raises(ValueError, match="not in DRAFT status"):
            manager.submit(["test-1"])

    def test_submit_empty_list(self, manager):
        """Test submitting empty list."""
        ids = manager.submit([])
        assert ids == []


class TestDeclarationManagerPay:
    def test_pay_single_declaration(self, manager, sample_declaration):
        """Test paying a single declaration."""
        ids = manager.pay(["test-1"])
        assert ids == ["test-1"]

        s = Storage()
        decl = s.get_declaration("test-1")
        assert decl.status == DeclarationStatus.PAID
        assert decl.paid_at is not None

    def test_pay_submitted_declaration(self, manager, sample_declaration):
        """Test paying a submitted declaration."""
        manager.submit(["test-1"])

        ids = manager.pay(["test-1"])
        assert ids == ["test-1"]

        s = Storage()
        decl = s.get_declaration("test-1")
        assert decl.status == DeclarationStatus.PAID

    def test_pay_empty_list(self, manager):
        """Test paying empty list."""
        ids = manager.pay([])
        assert ids == []

    def test_pay_multiple_declarations(self, manager, mock_user_data_dir):
        """Test paying multiple declarations."""
        s = Storage()

        for i in range(2):
            decl = Declaration(
                declaration_id=f"pay-{i}",
                type=DeclarationType.PPDG3R,
                status=DeclarationStatus.DRAFT,
                period_start=date(2024, 1, 1),
                period_end=date(2024, 6, 30),
                created_at=datetime(2024, 1, 1, 10, 0),
                file_path=str(s.declarations_dir / f"pay-{i}.xml"),
                xml_content="<xml>test</xml>",
                report_data=[],
            )
            s.save_declaration(decl)

        ids = manager.pay(["pay-0", "pay-1"])
        assert len(ids) == 2

    def test_pay_nonexistent(self, manager):
        """Test paying non-existent declaration."""
        with pytest.raises(ValueError, match="not found"):
            manager.pay(["nonexistent"])


class TestDeclarationManagerExport:
    def test_export_declaration(self, manager, sample_declaration, tmp_path):
        """Test exporting a declaration."""
        output_dir = tmp_path / "export"
        xml_path, attached_paths = manager.export("test-1", output_dir)

        assert xml_path.exists()
        assert xml_path.read_text() == "<xml>test content</xml>"
        assert attached_paths == []

    def test_export_with_attached_files(self, manager, sample_declaration, tmp_path):
        """Test exporting declaration with attached files."""
        s = Storage()

        # Attach a file
        test_file = tmp_path / "test_attach.txt"
        test_file.write_text("attached content")

        file_id = manager.attach_file("test-1", test_file)
        assert file_id == "test_attach.txt"

        # Export
        output_dir = tmp_path / "export"
        xml_path, attached_paths = manager.export("test-1", output_dir)

        assert xml_path.exists()
        assert len(attached_paths) == 1
        assert attached_paths[0].name == "test_attach.txt"
        assert attached_paths[0].read_text() == "attached content"

    def test_export_nonexistent(self, manager):
        """Test exporting non-existent declaration."""
        with pytest.raises(ValueError, match="not found"):
            manager.export("nonexistent")


class TestDeclarationManagerRevert:
    def test_revert_paid_to_draft(self, manager, sample_declaration):
        """Test reverting paid declaration to draft."""
        manager.pay(["test-1"])

        manager.revert(["test-1"], DeclarationStatus.DRAFT)

        s = Storage()
        decl = s.get_declaration("test-1")
        assert decl.status == DeclarationStatus.DRAFT
        assert decl.paid_at is None

    def test_revert_submitted_to_draft(self, manager, sample_declaration):
        """Test reverting submitted declaration to draft."""
        manager.submit(["test-1"])

        manager.revert(["test-1"], DeclarationStatus.DRAFT)

        s = Storage()
        decl = s.get_declaration("test-1")
        assert decl.status == DeclarationStatus.DRAFT
        assert decl.submitted_at is None

    def test_revert_multiple_declarations(self, manager, mock_user_data_dir):
        """Test reverting multiple declarations."""
        s = Storage()

        # Create and pay multiple declarations
        for i in range(2):
            decl = Declaration(
                declaration_id=f"revert-{i}",
                type=DeclarationType.PPDG3R,
                status=DeclarationStatus.DRAFT,
                period_start=date(2024, 1, 1),
                period_end=date(2024, 6, 30),
                created_at=datetime(2024, 1, 1, 10, 0),
                file_path=str(s.declarations_dir / f"revert-{i}.xml"),
                xml_content="<xml>test</xml>",
                report_data=[],
            )
            s.save_declaration(decl)

        manager.pay(["revert-0", "revert-1"])
        manager.revert(["revert-0", "revert-1"], DeclarationStatus.DRAFT)

        for i in range(2):
            decl = s.get_declaration(f"revert-{i}")
            assert decl.status == DeclarationStatus.DRAFT

    def test_revert_draft_fails(self, manager, sample_declaration):
        """Test that reverting draft declaration fails."""
        with pytest.raises(ValueError, match="Cannot revert"):
            manager.revert(["test-1"], DeclarationStatus.DRAFT)

    def test_revert_nonexistent(self, manager):
        """Test reverting non-existent declaration."""
        with pytest.raises(ValueError, match="not found"):
            manager.revert(["nonexistent"], DeclarationStatus.DRAFT)


class TestDeclarationManagerAttach:
    def test_attach_file(self, manager, sample_declaration, tmp_path):
        """Test attaching a file to declaration."""
        test_file = tmp_path / "test_file.txt"
        test_file.write_text("test content")

        file_id = manager.attach_file("test-1", test_file)
        assert file_id == "test_file.txt"

        s = Storage()
        decl = s.get_declaration("test-1")
        assert "test_file.txt" in decl.attached_files

        # Check file was copied
        attachments_dir = s.declarations_dir / "test-1" / "attachments"
        assert (attachments_dir / "test_file.txt").exists()
        assert (attachments_dir / "test_file.txt").read_text() == "test content"

    def test_attach_nonexistent_file(self, manager, sample_declaration):
        """Test attaching non-existent file."""
        with pytest.raises(ValueError, match="File not found"):
            manager.attach_file("test-1", Path("/nonexistent/file.txt"))

    def test_attach_nonexistent_declaration(self, manager, tmp_path):
        """Test attaching file to non-existent declaration."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("test")

        with pytest.raises(ValueError, match="not found"):
            manager.attach_file("nonexistent", test_file)


class TestDeclarationManagerDetach:
    def test_detach_file(self, manager, sample_declaration, tmp_path):
        """Test detaching a file from declaration."""
        # First attach
        test_file = tmp_path / "test_file.txt"
        test_file.write_text("test content")
        manager.attach_file("test-1", test_file)

        # Then detach
        manager.detach_file("test-1", "test_file.txt")

        s = Storage()
        decl = s.get_declaration("test-1")
        assert "test_file.txt" not in decl.attached_files

        # Check file was removed
        attachments_dir = s.declarations_dir / "test-1" / "attachments"
        assert not (attachments_dir / "test_file.txt").exists()

    def test_detach_nonexistent_file(self, manager, sample_declaration):
        """Test detaching non-existent file."""
        with pytest.raises(ValueError, match="not found"):
            manager.detach_file("test-1", "nonexistent.txt")

    def test_detach_nonexistent_declaration(self, manager):
        """Test detaching file from non-existent declaration."""
        with pytest.raises(ValueError, match="not found"):
            manager.detach_file("nonexistent", "file.txt")


def _apply_allure_labels() -> None:
    labels = (allure.epic("Tax"), allure.feature("Declaration Manager"))
    for name, value in list(globals().items()):
        if name.startswith("test_") and callable(value):
            decorated = value
            for label in labels:
                decorated = label(decorated)
            globals()[name] = decorated
        elif name.startswith("Test") and isinstance(value, type):
            decorated_class = value
            for label in labels:
                decorated_class = label(decorated_class)
            globals()[name] = decorated_class


_apply_allure_labels()
