"""Operation for importing data from CSV or Flex Query XML files."""

import xml.etree.ElementTree as ET
from datetime import date, datetime
from enum import Enum
from pathlib import Path

from ibkr_porez.ibkr_csv import CSVParser
from ibkr_porez.models import Transaction
from ibkr_porez.nbs import NBSClient
from ibkr_porez.operation_get import GetOperation
from ibkr_porez.storage import Storage


class ImportType(str, Enum):
    """Import file type."""

    CSV = "csv"
    FLEX = "flex"
    AUTO = "auto"


def _detect_file_type(file_path: Path) -> ImportType:
    """
    Auto-detect file type based on extension and content.

    Args:
        file_path: Path to the file

    Returns:
        ImportType: Detected file type
    """
    # Check extension first
    ext = file_path.suffix.lower()
    if ext == ".csv":
        return ImportType.CSV
    if ext in {".xml", ".zip"}:
        return ImportType.FLEX

    # Try to detect by content
    try:
        content = file_path.read_bytes()
        # Check for XML signature
        if content.startswith((b"<?xml", b"<FlexQueryResponse")):
            return ImportType.FLEX
        # Check for CSV-like content (has commas and newlines)
        if b"," in content[:1000] and b"\n" in content[:1000]:
            return ImportType.CSV
    except Exception:  # noqa: BLE001, S110
        # Silently fail and fall back to default
        pass

    # Default to CSV for unknown
    return ImportType.CSV


def _extract_date_from_xml(xml_content: str) -> date | None:
    """Extract report date from FlexStatement XML whenGenerated attribute."""
    date_length = 8  # YYYYMMDD format length

    try:
        root = ET.fromstring(xml_content)  # noqa: S314
        # Find FlexStatement element
        flex_stmt = root.find(".//FlexStatement")
        if flex_stmt is not None:
            when_generated = flex_stmt.get("whenGenerated")
            if when_generated:
                # Format: "YYYYMMDD;HHMMSS" or "YYYYMMDD"
                date_part = when_generated.split(";")[0]
                if len(date_part) == date_length:
                    return datetime.strptime(date_part, "%Y%m%d").date()
    except (ET.ParseError, ValueError, AttributeError):
        pass
    return None


class ImportOperation:
    """Operation for importing data from CSV or Flex Query XML files."""

    def __init__(self):
        self.storage = Storage()
        self.nbs = NBSClient(self.storage)
        self.get_operation = GetOperation(config=None)

    def execute(
        self,
        file_path: Path,
        import_type: ImportType = ImportType.AUTO,
    ) -> tuple[list[Transaction], int, int]:
        """
        Execute import operation.

        Args:
            file_path: Path to the file to import
            import_type: Type of import (CSV, FLEX, or AUTO for auto-detection)

        Returns:
            tuple[list[Transaction], int, int]: (transactions, count_inserted, count_updated)

        Raises:
            ValueError: If file cannot be parsed or import type is invalid
        """
        # Auto-detect if needed
        if import_type == ImportType.AUTO:
            import_type = _detect_file_type(file_path)

        if import_type == ImportType.CSV:
            return self._import_csv(file_path)
        if import_type == ImportType.FLEX:
            return self._import_flex(file_path)

        raise ValueError(f"Unknown import type: {import_type}")

    def _import_csv(self, file_path: Path) -> tuple[list[Transaction], int, int]:
        """Import from CSV file."""
        parser = CSVParser()
        with open(file_path, encoding="utf-8-sig") as f:
            transactions = parser.parse(f)

        if not transactions:
            return [], 0, 0

        # Save transactions
        count_inserted, count_updated = self.storage.save_transactions(transactions)

        # Sync exchange rates
        dates_to_fetch = set()
        for tx in transactions:
            dates_to_fetch.add((tx.date, tx.currency))
            if tx.open_date:
                dates_to_fetch.add((tx.open_date, tx.currency))

        for d, curr in dates_to_fetch:
            self.nbs.get_rate(d, curr)

        return transactions, count_inserted, count_updated

    def _import_flex(self, file_path: Path) -> tuple[list[Transaction], int, int]:
        """Import from Flex Query XML file."""
        # Read XML file
        xml_content = file_path.read_text(encoding="utf-8")

        # Extract date from XML (whenGenerated attribute) or use today
        report_date = _extract_date_from_xml(xml_content)
        if report_date is None:
            report_date = date.today()

        # Use GetOperation to process (same logic as get command)
        return self.get_operation.process_flex_query(xml_content, report_date)
