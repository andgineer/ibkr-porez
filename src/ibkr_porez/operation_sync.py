"""Operation for syncing data and creating declarations."""

from collections.abc import Callable
from dataclasses import dataclass
from datetime import date, datetime, timedelta
from decimal import ROUND_HALF_UP, Decimal
from pathlib import Path
from typing import Any

from ibkr_porez.config import UserConfig, config_manager
from ibkr_porez.models import (
    INCOME_CODE_DIVIDEND,
    Declaration,
    DeclarationStatus,
    DeclarationType,
    TaxReportEntry,
)
from ibkr_porez.operation_get import GetOperation
from ibkr_porez.report_gains import GainsReportGenerator
from ibkr_porez.report_income import IncomeReportGenerator
from ibkr_porez.storage import Storage


def _get_output_folder() -> Path:
    """Get output folder from config or default to Downloads."""
    config = config_manager.load_config()
    if config.output_folder:
        return Path(config.output_folder)
    return Path.home() / "Downloads"


@dataclass
class DeclarationConfig:
    """Configuration for a declaration type."""

    declaration_type: DeclarationType
    generator_factory: Callable[[], Any]
    period_getter: Callable[[], list[tuple[date, date, dict[str, Any]]]]
    declaration_id_generator: Callable[..., str] | None
    metadata_extractor: Callable[[list, date, date], dict[str, Any]]
    force: bool = False


class SyncOperation:
    """Operation for syncing data and creating declarations."""

    # Constants for half-year calculation
    JULY_MONTH = 7
    JUNE_MONTH = 6
    DEFAULT_FIRST_SYNC_LOOKBACK_DAYS = 45

    def __init__(
        self,
        config: UserConfig,
        output_dir: Path | None = None,
        forced_lookback_days: int | None = None,
    ):
        self.config = config
        self.storage = Storage()
        self.get_operation = GetOperation(config)
        self.output_dir = output_dir
        self.forced_lookback_days = forced_lookback_days

    def _get_next_declaration_number(self) -> int:
        """Get next sequential declaration number for all types."""
        existing_declarations = self.storage.get_declarations()
        return len(existing_declarations) + 1

    def _generate_declaration_filename(
        self,
        declaration_id: str,
        generator_filename: str,
    ) -> str:
        """Generate filename with declaration ID prefix."""
        # declaration_id is a string like "1", "2", etc.
        declaration_number = int(declaration_id)
        return f"{declaration_number:03d}-{generator_filename}"

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

    def _get_income_periods(
        self,
        start_period: date,
        end_period: date,
    ) -> list[tuple[date, date, dict]]:
        """Get periods to check for income declarations."""
        income_start = start_period + timedelta(days=1)
        income_end = end_period

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

    def _extract_gains_metadata(
        self,
        entries: list,
        period_start: date,
        period_end: date,
    ) -> dict:
        gains_entries = [e for e in entries if isinstance(e, TaxReportEntry)]
        total_positive_gain_rsd = sum(
            (e.capital_gain_rsd for e in gains_entries if e.capital_gain_rsd > 0),
            Decimal("0.00"),
        )
        total_losses_rsd = sum(
            (abs(e.capital_gain_rsd) for e in gains_entries if e.capital_gain_rsd < 0),
            Decimal("0.00"),
        )
        tax_base_rsd = max(Decimal("0.00"), total_positive_gain_rsd - total_losses_rsd)
        calculated_tax_rsd = (tax_base_rsd * Decimal("0.15")).quantize(
            Decimal("0.01"),
            rounding=ROUND_HALF_UP,
        )
        return {
            "entry_count": len(gains_entries),
            "period_start": period_start.isoformat(),
            "period_end": period_end.isoformat(),
            "total_gain_rsd": sum(e.capital_gain_rsd for e in gains_entries),
            "gross_income_rsd": total_positive_gain_rsd,
            "tax_base_rsd": tax_base_rsd,
            "calculated_tax_rsd": calculated_tax_rsd,
            "foreign_tax_paid_rsd": Decimal("0.00"),
            "tax_due_rsd": calculated_tax_rsd,
        }

    def _extract_income_metadata(
        self,
        entries: list,
        period_start: date,
        period_end: date,
    ) -> dict:
        if not entries:
            return {
                "entry_count": 0,
                "income_type": "unknown",
                "symbol": "unknown",
                "period_start": period_start.isoformat(),
                "period_end": period_end.isoformat(),
                "gross_income_rsd": Decimal("0.00"),
                "tax_base_rsd": Decimal("0.00"),
                "calculated_tax_rsd": Decimal("0.00"),
                "foreign_tax_paid_rsd": Decimal("0.00"),
                "tax_due_rsd": Decimal("0.00"),
            }

        # Extract from first entry
        first_entry = entries[0]
        symbol = str(getattr(first_entry, "symbol_or_currency", "")).strip().upper()
        period_start_date = min(
            (getattr(e, "date", period_start) for e in entries),
            default=period_start,
        )
        period_end_date = max(
            (getattr(e, "date", period_end) for e in entries),
            default=period_end,
        )
        gross_income_rsd = sum(
            (getattr(e, "bruto_prihod", Decimal("0.00")) for e in entries),
            Decimal("0.00"),
        )
        tax_base_rsd = sum(
            (getattr(e, "osnovica_za_porez", Decimal("0.00")) for e in entries),
            Decimal("0.00"),
        )
        calculated_tax_rsd = sum(
            (getattr(e, "obracunati_porez", Decimal("0.00")) for e in entries),
            Decimal("0.00"),
        )
        foreign_tax_paid_rsd = sum(
            (getattr(e, "porez_placen_drugoj_drzavi", Decimal("0.00")) for e in entries),
            Decimal("0.00"),
        )
        tax_due_rsd = sum(
            (getattr(e, "porez_za_uplatu", Decimal("0.00")) for e in entries),
            Decimal("0.00"),
        )

        # Determine income type from sifra_vrste_prihoda
        if hasattr(first_entry, "sifra_vrste_prihoda"):
            income_type = (
                "dividend" if first_entry.sifra_vrste_prihoda == INCOME_CODE_DIVIDEND else "coupon"
            )
        else:
            income_type = "dividend"  # Default

        return {
            "entry_count": len(entries),
            "income_type": income_type,
            "symbol": symbol or "unknown",
            "period_start": period_start_date.isoformat(),
            "period_end": period_end_date.isoformat(),
            "gross_income_rsd": gross_income_rsd,
            "tax_base_rsd": tax_base_rsd,
            "calculated_tax_rsd": calculated_tax_rsd,
            "foreign_tax_paid_rsd": foreign_tax_paid_rsd,
            "tax_due_rsd": tax_due_rsd,
        }

    def _create_declaration(  # noqa: PLR0913
        self,
        declaration_type: DeclarationType,
        declaration_id: str,
        period_start: date,
        period_end: date,
        generator_filename: str,
        xml_content: str,
        entries: list,
        metadata: dict,
    ) -> Declaration:
        """
        Create declaration from generator result.

        Args:
            declaration_type: Type of declaration
            declaration_id: Declaration ID (sequential number as string, e.g., "1", "2")
            period_start: Period start date
            period_end: Period end date
            generator_filename: Filename from generator (e.g., "ppdg3r-2023-H1.xml")
            xml_content: XML content string
            entries: Report entries
            metadata: Additional metadata

        Returns:
            Declaration object
        """
        # Generate proper filename with ID prefix
        proper_filename = self._generate_declaration_filename(
            declaration_id,
            generator_filename,
        )
        file_path = self.storage.declarations_dir / proper_filename
        with open(file_path, "w", encoding="utf-8") as f:
            f.write(xml_content)

        # Copy to output folder
        output_folder = self.output_dir if self.output_dir else _get_output_folder()
        output_folder.mkdir(parents=True, exist_ok=True)
        output_path = output_folder / proper_filename
        with open(output_path, "w", encoding="utf-8") as f:
            f.write(xml_content)

        return Declaration(
            declaration_id=declaration_id,
            type=declaration_type,
            status=DeclarationStatus.DRAFT,
            period_start=period_start,
            period_end=period_end,
            created_at=datetime.now(),
            file_path=str(file_path),
            xml_content=xml_content,
            report_data=entries,
            metadata=metadata,
        )

    def _process_declaration_type(  # noqa: PLR0912
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

        for period_start, period_end, _period_metadata in periods:
            try:
                generator = config.generator_factory()

                results = list(
                    generator.generate(
                        start_date=period_start,
                        end_date=period_end,
                        force=config.force,
                    ),
                )

                for filename, xml_content, entries in results:
                    metadata = config.metadata_extractor(entries, period_start, period_end)

                    # Check if declaration already exists by checking if any declaration
                    # has the same generator filename (without number prefix)
                    existing_declarations = self.storage.get_declarations(
                        declaration_type=config.declaration_type,
                    )
                    generator_base_name = Path(filename).stem
                    if any(
                        Path(d.file_path or "").stem.endswith(generator_base_name)
                        for d in existing_declarations
                        if d.file_path
                    ):
                        continue

                    declaration_id = str(self._get_next_declaration_number())

                    declaration = self._create_declaration(
                        declaration_type=config.declaration_type,
                        declaration_id=declaration_id,
                        period_start=date.fromisoformat(
                            str(metadata.get("period_start") or period_start),
                        ),
                        period_end=date.fromisoformat(
                            str(metadata.get("period_end") or period_end),
                        ),
                        generator_filename=filename,
                        xml_content=xml_content,
                        entries=entries,
                        metadata=metadata,
                    )

                    self.storage.save_declaration(declaration)
                    created_declarations.append(declaration)

            except ValueError as e:
                error_msg = str(e)
                # For gains: skip if no taxable sales
                if config.declaration_type == DeclarationType.PPDG3R:
                    continue
                # For income: skip if no income found (normal case)
                # But raise error for other issues (tax not found, etc.)
                if "No income (dividends/coupons) found in this period" in error_msg:
                    continue
                # For other errors (tax not found, etc.): raise
                raise ValueError(
                    f"Error creating {config.declaration_type.value} declarations: {e}. "
                    "Fix the issue and run sync again.",
                ) from e

        return created_declarations

    def _get_declaration_configs(
        self,
        end_period: date,
    ) -> list[DeclarationConfig]:
        """
        Get declaration configurations for all declaration types.

        Args:
            end_period: End period for this sync run

        Returns:
            List of declaration configurations
        """
        if self.forced_lookback_days is not None:
            start_period = end_period - timedelta(days=self.forced_lookback_days - 1)
        else:
            start_period = self.storage.get_last_declaration_date() or end_period - timedelta(
                days=self.DEFAULT_FIRST_SYNC_LOOKBACK_DAYS - 1,
            )

        return [
            DeclarationConfig(
                declaration_type=DeclarationType.PPDG3R,
                generator_factory=GainsReportGenerator,
                period_getter=lambda: [
                    (
                        start_date,
                        end_date,
                        {},  # No metadata needed
                    )
                    for start_date, end_date, _year, _half in [self._get_last_complete_half_year()]
                ],
                declaration_id_generator=None,  # Not needed, sequential number is used
                metadata_extractor=self._extract_gains_metadata,
                force=False,
            ),
            DeclarationConfig(
                declaration_type=DeclarationType.PPO,
                generator_factory=IncomeReportGenerator,
                period_getter=lambda: self._get_income_periods(
                    start_period,
                    end_period,
                ),
                declaration_id_generator=None,  # Not needed, filename is used as ID
                metadata_extractor=self._extract_income_metadata,
                force=False,
            ),
        ]

    def execute(self) -> list[Declaration]:
        """
        Execute sync operation.

        Returns:
            list[Declaration]: List of newly created declarations
        """
        self.get_operation.execute()

        # IBKR Flex Query data appears with 1-2 day delay
        today = datetime.now().date()
        end_period = today - timedelta(days=1)
        saved_start_period = self.storage.get_last_declaration_date()

        declaration_configs = self._get_declaration_configs(
            end_period,
        )

        created_declarations = []
        for config in declaration_configs:
            declarations = self._process_declaration_type(config)
            created_declarations.extend(declarations)

        # Update last_declaration_date only if all declarations created successfully
        # Update if declarations were created OR if last_declaration_date is None or in the past
        if created_declarations or saved_start_period is None or saved_start_period < end_period:
            self.storage.set_last_declaration_date(end_period)

        return created_declarations
