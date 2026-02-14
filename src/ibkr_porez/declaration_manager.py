"""Declaration management operations."""

import shutil
from datetime import datetime
from decimal import Decimal, InvalidOperation
from pathlib import Path

from ibkr_porez.config import config_manager
from ibkr_porez.models import Declaration, DeclarationStatus, DeclarationType
from ibkr_porez.storage import Storage


class DeclarationManager:
    """Manager for declaration status operations."""

    def __init__(self):
        self.storage = Storage()

    @staticmethod
    def is_transition_allowed(
        current_status: DeclarationStatus,
        target_status: DeclarationStatus,
    ) -> bool:
        """Return whether status transition is allowed."""
        if current_status == target_status:
            return False
        if target_status == DeclarationStatus.SUBMITTED:
            return current_status == DeclarationStatus.DRAFT
        if target_status == DeclarationStatus.PENDING:
            return current_status in (DeclarationStatus.DRAFT, DeclarationStatus.SUBMITTED)
        if target_status == DeclarationStatus.FINALIZED:
            return current_status in (
                DeclarationStatus.DRAFT,
                DeclarationStatus.SUBMITTED,
                DeclarationStatus.PENDING,
            )
        if target_status == DeclarationStatus.DRAFT:
            return current_status in (
                DeclarationStatus.SUBMITTED,
                DeclarationStatus.PENDING,
                DeclarationStatus.FINALIZED,
            )
        return False

    @staticmethod
    def tax_due_rsd(declaration: Declaration) -> Decimal:
        """
        Return tax due from declaration metadata.

        Missing/invalid metadata is treated as non-zero to avoid accidental auto-finalization.
        """
        value = declaration.metadata.get("assessed_tax_due_rsd")
        if value is None:
            value = declaration.metadata.get("tax_due_rsd")
        if isinstance(value, Decimal):
            return value
        if value is None:
            return Decimal("1.00")
        try:
            return Decimal(str(value))
        except (InvalidOperation, TypeError, ValueError):
            return Decimal("1.00")

    @classmethod
    def has_tax_to_pay(cls, declaration: Declaration) -> bool:
        """Return whether declaration has positive tax due."""
        return cls.tax_due_rsd(declaration) > Decimal("0")

    @staticmethod
    def has_assessed_tax(declaration: Declaration) -> bool:
        """Return whether declaration has official assessed tax amount."""
        value = declaration.metadata.get("assessed_tax_due_rsd")
        if value is None:
            return False
        try:
            return Decimal(str(value)) >= Decimal("0")
        except (InvalidOperation, TypeError, ValueError):
            return False

    def submit(
        self,
        declaration_ids: list[str],
    ) -> list[str]:
        """
        Mark declaration(s) as submitted.

        Args:
            declaration_ids: List of declaration IDs to submit

        Returns:
            list[str]: List of declaration IDs that were submitted
        """
        submitted_ids = []
        timestamp = datetime.now()

        for declaration_id in declaration_ids:
            decl = self.storage.get_declaration(declaration_id)
            if not decl:
                raise ValueError(f"Declaration {declaration_id} not found")
            if decl.status != DeclarationStatus.DRAFT:
                raise ValueError(
                    f"Declaration {declaration_id} is not in DRAFT status (current: {decl.status})",
                )
            if decl.type == DeclarationType.PPDG3R:
                target_status = DeclarationStatus.PENDING
            else:
                target_status = (
                    DeclarationStatus.SUBMITTED
                    if self.has_tax_to_pay(decl)
                    else DeclarationStatus.FINALIZED
                )
            self.storage.update_declaration_status(declaration_id, target_status, timestamp)
            if target_status != DeclarationStatus.SUBMITTED:
                updated = self.storage.get_declaration(declaration_id)
                if updated is not None and updated.submitted_at is None:
                    updated.submitted_at = timestamp
                    self.storage.save_declaration(updated)
            submitted_ids.append(declaration_id)

        return submitted_ids

    def apply_status(
        self,
        declaration_ids: list[str],
        target_status: DeclarationStatus,
    ) -> list[str]:
        """Apply target status using the canonical status-operation mapping."""
        if target_status == DeclarationStatus.DRAFT:
            self.revert(declaration_ids, DeclarationStatus.DRAFT)
            return declaration_ids
        if target_status == DeclarationStatus.SUBMITTED:
            return self.submit(declaration_ids)
        if target_status == DeclarationStatus.FINALIZED:
            return self.pay(declaration_ids)
        raise ValueError(f"Unsupported status: {target_status}")

    def pay(
        self,
        declaration_ids: list[str],
    ) -> list[str]:
        """
        Mark declaration(s) as finalized.

        Args:
            declaration_ids: List of declaration IDs to pay

        Returns:
            list[str]: List of declaration IDs that were marked finalized
        """
        paid_ids = []
        timestamp = datetime.now()

        for declaration_id in declaration_ids:
            decl = self.storage.get_declaration(declaration_id)
            if not decl:
                raise ValueError(f"Declaration {declaration_id} not found")
            if decl.status not in (
                DeclarationStatus.DRAFT,
                DeclarationStatus.SUBMITTED,
                DeclarationStatus.PENDING,
            ):
                raise ValueError(
                    f"Declaration {declaration_id} cannot be paid (current: {decl.status})",
                )
            self.storage.update_declaration_status(
                declaration_id,
                DeclarationStatus.FINALIZED,
                timestamp,
            )
            updated_decl = self.storage.get_declaration(declaration_id)
            if updated_decl is not None:
                updated_decl.paid_at = timestamp
                self.storage.save_declaration(updated_decl)
            paid_ids.append(declaration_id)

        return paid_ids

    def set_assessed_tax(
        self,
        declaration_id: str,
        tax_due_rsd: Decimal,
        mark_paid: bool = False,
    ) -> Declaration:
        """Set official assessed tax amount and update declaration status."""
        decl = self.storage.get_declaration(declaration_id)
        if not decl:
            raise ValueError(f"Declaration {declaration_id} not found")
        if decl.status == DeclarationStatus.DRAFT:
            raise ValueError(f"Declaration {declaration_id} must be submitted before assessment")
        if tax_due_rsd < Decimal("0"):
            raise ValueError("Assessed tax cannot be negative")

        timestamp = datetime.now()
        decl.metadata["assessed_tax_due_rsd"] = tax_due_rsd
        decl.metadata["tax_due_rsd"] = tax_due_rsd

        if mark_paid:
            decl.status = DeclarationStatus.FINALIZED
            decl.paid_at = timestamp
        elif tax_due_rsd > Decimal("0"):
            decl.status = DeclarationStatus.SUBMITTED
            decl.paid_at = None
        else:
            decl.status = DeclarationStatus.FINALIZED
            decl.paid_at = None

        if decl.submitted_at is None:
            decl.submitted_at = timestamp

        self.storage.save_declaration(decl)
        return decl

    def export(
        self,
        declaration_id: str,
        output_dir: Path | None = None,
    ) -> tuple[Path, list[Path]]:
        """
        Export declaration XML and all attached files.

        Args:
            declaration_id: Declaration ID
            output_dir: Output directory (default: from config or Downloads)

        Returns:
            tuple[Path, list[Path]]: (xml_file_path, list of attached file paths)
        """
        decl = self.storage.get_declaration(declaration_id)
        if not decl:
            raise ValueError(f"Declaration {declaration_id} not found")

        if output_dir is None:
            config = config_manager.load_config()
            if config.output_folder:
                output_dir = Path(config.output_folder)
            else:
                output_dir = Path.home() / "Downloads"
        else:
            output_dir = Path(output_dir)

        output_dir.mkdir(parents=True, exist_ok=True)

        # Export XML file
        if not decl.xml_content and decl.file_path:
            xml_content = Path(decl.file_path).read_text(encoding="utf-8")
        elif decl.xml_content:
            xml_content = decl.xml_content
        else:
            raise ValueError(f"Declaration {declaration_id} has no XML content")

        xml_filename = Path(decl.file_path).name if decl.file_path else f"{declaration_id}.xml"
        xml_output_path = output_dir / xml_filename
        xml_output_path.write_text(xml_content, encoding="utf-8")

        # Export attached files
        attached_paths = []
        if decl.attached_files:
            for file_identifier, relative_path in decl.attached_files.items():
                # Construct full path to source file
                source_path = self.storage.declarations_dir / relative_path
                if source_path.exists():
                    # Copy to output_dir with original filename
                    dest_path = output_dir / file_identifier
                    shutil.copy2(source_path, dest_path)
                    attached_paths.append(dest_path)

        return xml_output_path, attached_paths

    def revert(
        self,
        declaration_ids: list[str],
        target_status: DeclarationStatus,
    ) -> None:
        """
        Revert declaration status (e.g., finalized -> draft).

        Args:
            declaration_ids: List of declaration IDs to revert
            target_status: Target status (usually DRAFT)
        """
        for declaration_id in declaration_ids:
            decl = self.storage.get_declaration(declaration_id)
            if not decl:
                raise ValueError(f"Declaration {declaration_id} not found")

            # Validate transition
            if target_status == DeclarationStatus.DRAFT:
                if decl.status not in (
                    DeclarationStatus.SUBMITTED,
                    DeclarationStatus.PENDING,
                    DeclarationStatus.FINALIZED,
                ):
                    raise ValueError(
                        f"Cannot revert declaration {declaration_id} "
                        f"from {decl.status} to {target_status}",
                    )
                # Clear timestamps when reverting to draft
                timestamp = datetime.now()
                self.storage.update_declaration_status(declaration_id, target_status, timestamp)
                decl = self.storage.get_declaration(declaration_id)
                if decl:
                    decl.submitted_at = None
                    decl.paid_at = None
                    self.storage.save_declaration(decl)
            else:
                raise ValueError(f"Revert to {target_status} is not supported")

    def attach_file(
        self,
        declaration_id: str,
        file_path: Path,
    ) -> str:
        """
        Attach file to declaration.

        Args:
            declaration_id: Declaration ID
            file_path: Path to file to attach

        Returns:
            str: File identifier (filename) used as unique ID within declaration
        """
        decl = self.storage.get_declaration(declaration_id)
        if not decl:
            raise ValueError(f"Declaration {declaration_id} not found")

        source_path = Path(file_path)
        if not source_path.exists():
            raise ValueError(f"File not found: {file_path}")

        # Use filename as identifier
        file_identifier = source_path.name

        # Create attachments directory
        attachments_dir = self.storage.declarations_dir / declaration_id / "attachments"
        attachments_dir.mkdir(parents=True, exist_ok=True)

        # Copy file to attachments directory
        dest_path = attachments_dir / file_identifier
        shutil.copy2(source_path, dest_path)

        # Calculate relative path from declarations_dir
        relative_path = dest_path.relative_to(self.storage.declarations_dir)

        # Update declaration
        decl.attached_files[file_identifier] = str(relative_path)
        self.storage.save_declaration(decl)

        return file_identifier

    def detach_file(
        self,
        declaration_id: str,
        file_identifier: str,
    ) -> None:
        """
        Remove attached file from declaration.

        Args:
            declaration_id: Declaration ID
            file_identifier: File identifier (filename) to remove
        """
        decl = self.storage.get_declaration(declaration_id)
        if not decl:
            raise ValueError(f"Declaration {declaration_id} not found")

        if file_identifier not in decl.attached_files:
            raise ValueError(f"File '{file_identifier}' not found in declaration {declaration_id}")

        # Get file path and remove from filesystem
        relative_path = decl.attached_files[file_identifier]
        file_path = self.storage.declarations_dir / relative_path
        if file_path.exists():
            file_path.unlink()

        # Remove from declaration
        del decl.attached_files[file_identifier]
        self.storage.save_declaration(decl)
