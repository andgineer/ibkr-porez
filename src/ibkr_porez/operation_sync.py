"""Operation for syncing data and creating declarations."""

from collections.abc import Callable
from dataclasses import dataclass
from datetime import date, datetime, timedelta
from pathlib import Path
from typing import Any

from ibkr_porez.config import UserConfig
from ibkr_porez.models import (
    Declaration,
    DeclarationStatus,
    DeclarationType,
    TaxReportEntry,
)
from ibkr_porez.operation_get import GetOperation
from ibkr_porez.report_gains import GainsReportGenerator
from ibkr_porez.report_income import IncomeReportGenerator
from ibkr_porez.storage import Storage


@dataclass
class DeclarationConfig:
    """Configuration for a declaration type."""

    declaration_type: DeclarationType
    generator_factory: Callable[[], Any]
    period_getter: Callable[[], list[tuple[date, date, dict[str, Any]]]]
    declaration_id_generator: Callable[..., str]
    metadata_extractor: Callable[..., dict[str, Any]]
    force: bool = False


class SyncOperation:
    """Operation for syncing data and creating declarations."""

    # Constants for half-year calculation
    JULY_MONTH = 7
    JUNE_MONTH = 6
    MIN_FILENAME_PARTS = 3

    def __init__(self, config: UserConfig):
        self.config = config
        self.storage = Storage()
        self.get_operation = GetOperation(config)

    def _generate_declaration_id_gains(self, year: int, half: int) -> str:
        """Generate declaration ID for PPDG-3R."""
        return f"ppdg3r_{year}_H{half}"

    def _generate_declaration_id_income(
        self,
        declaration_date: date,
        symbol_or_currency: str,
        income_type: str,
    ) -> str:
        """Generate declaration ID for PP OPO."""
        return f"ppopo_{declaration_date}_{symbol_or_currency}_{income_type}"

    def _generate_declaration_filename(
        self,
        declaration_type: DeclarationType,
        declaration_date: date,
        symbol_or_currency: str | None = None,
    ) -> str:
        """
        Generate declaration filename in format: nnnn-ppopo-voo-yyyy-mmdd.xml

        Args:
            declaration_type: Type of declaration
            declaration_date: Date of declaration
            symbol_or_currency: Symbol (for dividends) or currency (for interest)

        Returns:
            Filename string
        """
        # Get next declaration number for this type
        existing_declarations = self.storage.get_declarations(declaration_type=declaration_type)
        declaration_number = len(existing_declarations) + 1

        if declaration_type == DeclarationType.PPO:
            # Format: nnnn-ppopo-voo-yyyy-mmdd.xml
            symbol_lower = (symbol_or_currency or "unknown").lower()
            date_str = declaration_date.strftime("%Y-%m%d")
            return f"{declaration_number:04d}-ppopo-{symbol_lower}-{date_str}.xml"
        # Format: nnnn-ppdg3r-yyyy-Hh.xml
        year = declaration_date.year
        # Determine half from date
        half = 1 if declaration_date.month <= self.JUNE_MONTH else 2
        return f"{declaration_number:04d}-ppdg3r-{year}-H{half}.xml"

    def _get_last_complete_half_year(self) -> tuple[date, date, int, int]:
        """
        Get last complete half-year period.

        Returns:
            tuple: (start_date, end_date, year, half)
        """
        now = datetime.now().date()
        current_year = now.year
        current_month = now.month

        if current_month < self.JULY_MONTH:
            # Current is H1 (incomplete), so Last Complete is Previous Year H2
            year = current_year - 1
            half = 2
            start_date = date(year, 7, 1)
            end_date = date(year, 12, 31)
        else:
            # Current is H2 (incomplete), so Last Complete is Current Year H1
            year = current_year
            half = 1
            start_date = date(year, 1, 1)
            end_date = date(year, 6, 30)

        return start_date, end_date, year, half

    def _get_gains_periods(self) -> list[tuple[date, date, dict]]:
        """Get periods to check for gains declarations."""
        start_date, end_date, year, half = self._get_last_complete_half_year()
        declaration_id = self._generate_declaration_id_gains(year, half)

        if self.storage.declaration_exists(declaration_id):
            return []

        return [
            (
                start_date,
                end_date,
                {
                    "declaration_id": declaration_id,
                    "year": year,
                    "half": half,
                },
            ),
        ]

    def _get_income_periods(
        self,
        last_declaration_date: date,
        new_last_declaration_date: date,
    ) -> list[tuple[date, date, dict]]:
        """Get periods to check for income declarations."""
        income_start = last_declaration_date + timedelta(days=1)
        income_end = new_last_declaration_date

        if income_start > income_end:
            return []

        # Return single period - generator will handle grouping by date/symbol
        return [
            (
                income_start,
                income_end,
                {
                    "period_start": income_start,
                    "period_end": income_end,
                },
            ),
        ]

    def _extract_gains_metadata(self, entries: list) -> dict:
        """Extract metadata from gains entries."""
        gains_entries = [e for e in entries if isinstance(e, TaxReportEntry)]
        return {
            "entry_count": len(gains_entries),
            "total_gain": sum(e.capital_gain_rsd for e in gains_entries),
        }

    def _extract_income_metadata(
        self,
        entries: list,
        _declaration_date: date,
        symbol_or_currency: str,
        income_type: str,
    ) -> dict:
        """Extract metadata from income entries."""
        return {
            "entry_count": len(entries),
            "income_type": income_type,
            "symbol_or_currency": symbol_or_currency,
        }

    def _parse_income_filename(self, filename: str, fallback_date: date) -> tuple[date, str, str]:
        """
        Parse income declaration info from filename.

        Args:
            filename: Temp filename from generator
            fallback_date: Fallback date if parsing fails

        Returns:
            tuple: (declaration_date, symbol_or_currency, income_type)
        """
        # Format: ppopo_YYYY-MM-DD_symbol_income_type.xml
        # Extract only filename, not full path
        filename_only = Path(filename).name.replace(".xml", "")
        parts = filename_only.split("_")
        if len(parts) >= self.MIN_FILENAME_PARTS:
            declaration_date_str = parts[1]
            symbol_or_currency = parts[2]
            income_type = (
                parts[self.MIN_FILENAME_PARTS]
                if len(parts) > self.MIN_FILENAME_PARTS
                else "dividend"
            )
            declaration_date = date.fromisoformat(declaration_date_str)
        else:
            # Fallback parsing
            declaration_date = fallback_date
            symbol_or_currency = "unknown"
            income_type = "dividend"

        return declaration_date, symbol_or_currency, income_type

    def _create_declaration(  # noqa: PLR0913
        self,
        declaration_type: DeclarationType,
        declaration_id: str,
        period_start: date,
        period_end: date,
        temp_filename: str,
        entries: list,
        metadata: dict,
        symbol_or_currency: str | None = None,
    ) -> Declaration:
        """
        Create declaration from generator result.

        Args:
            declaration_type: Type of declaration
            declaration_id: Declaration ID
            period_start: Period start date
            period_end: Period end date
            temp_filename: Temporary filename from generator
            entries: Report entries
            metadata: Additional metadata
            symbol_or_currency: Symbol or currency for filename generation

        Returns:
            Declaration object
        """
        # Read XML content from temp file
        with open(temp_filename, encoding="utf-8") as f:
            xml_content = f.read()

        # Generate proper filename with number
        proper_filename = self._generate_declaration_filename(
            declaration_type,
            period_end if declaration_type == DeclarationType.PPDG3R else period_start,
            symbol_or_currency,
        )

        # Write to proper filename
        with open(proper_filename, "w", encoding="utf-8") as f:
            f.write(xml_content)

        # Remove temp file
        Path(temp_filename).unlink(missing_ok=True)

        # Create Declaration
        return Declaration(
            declaration_id=declaration_id,
            type=declaration_type,
            status=DeclarationStatus.DRAFT,
            period_start=period_start,
            period_end=period_end,
            created_at=datetime.now(),
            file_path=proper_filename,
            xml_content=xml_content,
            report_data=entries,
            metadata=metadata,
        )

    def _process_declaration_type(
        self,
        config: DeclarationConfig,
    ) -> list[Declaration]:
        """
        Process a declaration type and create declarations.

        Args:
            config: Declaration configuration

        Returns:
            List of created declarations
        """
        created_declarations = []

        # Get periods to check
        periods = config.period_getter()

        for period_start, period_end, period_metadata in periods:
            try:
                # Create generator
                generator = config.generator_factory()

                # Generate declarations
                # Note: generators may accept different parameters
                if config.declaration_type == DeclarationType.PPDG3R:
                    generator_kwargs = {
                        "start_date": period_start,
                        "end_date": period_end,
                    }
                else:
                    generator_kwargs = {
                        "start_date": period_start,
                        "end_date": period_end,
                        "force": config.force,
                    }

                results = list(generator.generate(**generator_kwargs))

                for temp_filename, entries in results:
                    # Extract declaration info based on type
                    if config.declaration_type == DeclarationType.PPDG3R:
                        declaration_id = period_metadata["declaration_id"]
                        symbol_or_currency = None
                        # Gains metadata extractor only needs entries
                        metadata = config.metadata_extractor(entries)
                    else:
                        # Income: parse from filename
                        declaration_date, symbol_or_currency, income_type = (
                            self._parse_income_filename(temp_filename, period_start)
                        )
                        declaration_id = config.declaration_id_generator(
                            declaration_date,
                            symbol_or_currency,
                            income_type,
                        )
                        # Income metadata extractor needs entries and additional params
                        metadata = config.metadata_extractor(
                            entries,
                            declaration_date,
                            symbol_or_currency,
                            income_type,
                        )

                    # Check if declaration already exists
                    if self.storage.declaration_exists(declaration_id):
                        Path(temp_filename).unlink(missing_ok=True)
                        continue

                    # Create declaration
                    declaration = self._create_declaration(
                        declaration_type=config.declaration_type,
                        declaration_id=declaration_id,
                        period_start=period_start,
                        period_end=period_end,
                        temp_filename=temp_filename,
                        entries=entries,
                        metadata=metadata,
                        symbol_or_currency=symbol_or_currency,
                    )

                    self.storage.save_declaration(declaration)
                    created_declarations.append(declaration)

            except ValueError as e:
                # For gains: skip if no taxable sales
                if config.declaration_type == DeclarationType.PPDG3R:
                    continue
                # For income: raise error (tax not found, etc.)
                raise ValueError(
                    f"Error creating {config.declaration_type.value} declarations: {e}. "
                    "Fix the issue and run sync again.",
                ) from e

        return created_declarations

    def execute(self) -> list[Declaration]:
        """
        Execute sync operation.

        Returns:
            list[Declaration]: List of newly created declarations
        """
        # 1. Fetch fresh data from IBKR
        self.get_operation.execute()

        # 2. Calculate new_last_declaration_date (previous day)
        # IBKR Flex Query data appears with 1-2 day delay
        today = datetime.now().date()
        new_last_declaration_date = today - timedelta(days=1)

        # Get current last_declaration_date
        last_declaration_date = self.storage.get_last_declaration_date()
        if last_declaration_date is None:
            # First sync: set to 30 days ago
            last_declaration_date = today - timedelta(days=30)

        # 3. Process declaration types
        declaration_configs = [
            DeclarationConfig(
                declaration_type=DeclarationType.PPDG3R,
                generator_factory=GainsReportGenerator,
                period_getter=self._get_gains_periods,
                declaration_id_generator=self._generate_declaration_id_gains,
                metadata_extractor=self._extract_gains_metadata,
                force=False,
            ),
            DeclarationConfig(
                declaration_type=DeclarationType.PPO,
                generator_factory=IncomeReportGenerator,
                period_getter=lambda: self._get_income_periods(
                    last_declaration_date,
                    new_last_declaration_date,
                ),
                declaration_id_generator=self._generate_declaration_id_income,
                metadata_extractor=self._extract_income_metadata,
                force=False,
            ),
        ]

        created_declarations = []
        for config in declaration_configs:
            declarations = self._process_declaration_type(config)
            created_declarations.extend(declarations)

        # 4. Update last_declaration_date only if all declarations created successfully
        # Update if declarations were created OR if last_declaration_date is None or in the past
        if (
            created_declarations
            or last_declaration_date is None
            or last_declaration_date < new_last_declaration_date
        ):
            self.storage.set_last_declaration_date(new_last_declaration_date)

        return created_declarations
