"""Generator for PP OPO (Capital Income) reports."""

import re
from collections import defaultdict
from datetime import date, timedelta
from decimal import ROUND_HALF_UP, Decimal

import pandas as pd

from ibkr_porez.config import config_manager
from ibkr_porez.declaration_income_xml import IncomeXMLGenerator
from ibkr_porez.models import Currency, IncomeDeclarationEntry, IncomeEntry, TransactionType
from ibkr_porez.nbs import NBSClient
from ibkr_porez.storage import Storage


class IncomeReportGenerator:
    """Generator for PP OPO (Capital Income) reports."""

    def __init__(self):
        self.cfg = config_manager.load_config()
        self.storage = Storage()
        self.nbs = NBSClient(self.storage)
        self.xml_gen = IncomeXMLGenerator(self.cfg)

    def _parse_entity_from_description(self, description: str) -> tuple[str | None, str | None]:
        """
        Parse entity_name and ISIN from description.

        Pattern: "SYMBOL (ISIN)" -> entity_name="SYMBOL", entity_isin="ISIN"
        Example: "SGOV (US78463A1034)" -> ("SGOV", "US78463A1034")

        Args:
            description: Transaction description.

        Returns:
            tuple: (entity_name, entity_isin) or (None, None) if not found.
        """
        # Regex pattern: ^([0-9A-Za-z\.]+)\s*\(([0-9A-Za-z]+)\)
        pattern = r"^([0-9A-Za-z\.]+)\s*\(([0-9A-Za-z]+)\)"
        match = re.match(pattern, description.strip())
        if match:
            return match.group(1), match.group(2)
        return None, None

    def _match_withholding_tax_row(  # noqa: PLR0913
        self,
        row: pd.Series,
        income_type: str,
        symbol: str,
        currency: Currency,
        entity_name: str | None,
        entity_isin: str | None,
    ) -> bool:
        """
        Check if a withholding tax row matches the income criteria.

        Args:
            row: DataFrame row with withholding tax transaction.
            income_type: Type of income ("dividend" or "coupon").
            symbol: Symbol of income transaction.
            currency: Currency of income transaction.
            entity_name: Parsed entity name from income description (for dividends).
            entity_isin: Parsed ISIN from income description (for dividends).

        Returns:
            True if row matches, False otherwise.
        """
        tx_symbol = str(row["symbol"]) if pd.notna(row["symbol"]) else "UNKNOWN"
        tx_currency_str = str(row["currency"])
        tx_description = str(row.get("description", "")) if pd.notna(row.get("description")) else ""

        if income_type == "dividend":
            # Try entity_name/ISIN match first
            if entity_name and entity_isin:
                tx_entity_name, tx_entity_isin = self._parse_entity_from_description(
                    tx_description,
                )
                if tx_entity_name == entity_name and tx_entity_isin == entity_isin:
                    return True
            # Fallback to symbol match
            return tx_symbol == symbol

        if income_type == "coupon":
            # For interest: match by currency
            try:
                tx_currency = Currency(tx_currency_str)
                return tx_currency == currency
            except ValueError:
                return False

        return False

    def _find_withholding_tax(  # noqa: PLR0913
        self,
        income_date: date,
        symbol: str,
        income_type: str,
        currency: Currency,
        income_entries: list[IncomeEntry],
        withholding_df: pd.DataFrame,
        max_days_offset: int = 7,
    ) -> Decimal:
        """
        Find withholding tax for income transaction, searching in subsequent days.

        Args:
            income_date: Date of income transaction.
            symbol: Symbol of income transaction (or currency code for interest).
            income_type: Type of income ("dividend" or "coupon").
            currency: Currency of income transaction.
            income_entries: List of all income entries (for matching by entity_name/ISIN).
            withholding_df: DataFrame with all withholding tax transactions.
            max_days_offset: Maximum number of days to search after income date.

        Returns:
            Total withholding tax in RSD, or Decimal("0.00") if not found.
        """
        total_tax_rsd = Decimal("0.00")

        # Search in range: income_date to income_date + max_days_offset
        search_end_date = income_date + timedelta(days=max_days_offset)

        # For dividends: try to parse entity_name and ISIN from description
        entity_name = None
        entity_isin = None
        if income_type == "dividend" and income_entries:
            first_entry = income_entries[0]
            entity_name, entity_isin = self._parse_entity_from_description(first_entry.description)

        for _, row in withholding_df.iterrows():
            tx_date = pd.to_datetime(row["date"]).date()

            # Only consider taxes on or after income date, up to max_days_offset
            if tx_date < income_date or tx_date > search_end_date:
                continue

            # Check if row matches
            if not self._match_withholding_tax_row(
                row,
                income_type,
                symbol,
                currency,
                entity_name,
                entity_isin,
            ):
                continue

            amount = Decimal(str(row["amount"]))
            currency_str = str(row["currency"])

            # WITHHOLDING_TAX is negative, we need absolute value
            tax_amount = abs(amount)

            try:
                tx_currency = Currency(currency_str)
            except ValueError:
                continue

            rate = self.nbs.get_rate(tx_date, tx_currency)
            if not rate:
                continue

            tax_rsd = tax_amount * rate
            total_tax_rsd += tax_rsd

        return total_tax_rsd

    def _process_income_transactions(  # noqa: C901
        self,
        df_transactions: pd.DataFrame,
        start_date: date,
        end_date: date,
    ) -> tuple[list[IncomeEntry], pd.DataFrame]:
        """
        Process DIVIDEND and INTEREST transactions and convert to IncomeEntry.
        Also collect WITHHOLDING_TAX transactions for later matching.

        Args:
            df_transactions: DataFrame with all transactions.
            start_date: Start date for filtering.
            end_date: End date for filtering.

        Returns:
            tuple: (list[IncomeEntry], withholding_df)
        """
        if df_transactions.empty:
            return [], pd.DataFrame()

        # Filter for DIVIDEND and INTEREST in period
        income_types = [TransactionType.DIVIDEND.value, TransactionType.INTEREST.value]
        income_df = df_transactions[
            (df_transactions["type"].isin(income_types))
            & (pd.to_datetime(df_transactions["date"]).dt.date >= start_date)
            & (pd.to_datetime(df_transactions["date"]).dt.date <= end_date)
        ].copy()

        if income_df.empty:
            return [], pd.DataFrame()

        # Get all WITHHOLDING_TAX transactions (we'll search in extended range)
        # Search up to max_days_offset days after end_date to catch late taxes
        max_days_offset = 7
        withholding_search_end = end_date + timedelta(days=max_days_offset)
        withholding_df = df_transactions[
            (df_transactions["type"] == TransactionType.WITHHOLDING_TAX.value)
            & (pd.to_datetime(df_transactions["date"]).dt.date >= start_date)
            & (pd.to_datetime(df_transactions["date"]).dt.date <= withholding_search_end)
        ].copy()

        entries = []

        for _, row in income_df.iterrows():
            tx_date = pd.to_datetime(row["date"]).date()
            symbol = str(row["symbol"]) if pd.notna(row["symbol"]) else "UNKNOWN"
            amount = Decimal(str(row["amount"]))
            currency_str = str(row["currency"])

            # Determine income type
            tx_type = str(row["type"])
            if tx_type == TransactionType.DIVIDEND.value:
                income_type = "dividend"
            elif tx_type == TransactionType.INTEREST.value:
                income_type = "coupon"
            else:
                continue  # Skip unknown types

            # Get exchange rate
            try:
                currency = Currency(currency_str)
            except ValueError:
                continue  # Skip unknown currency

            rate = self.nbs.get_rate(tx_date, currency)
            if not rate:
                continue  # Skip if rate not available

            amount_rsd = amount * rate

            # Get description
            description = (
                str(row.get("description", "")) if pd.notna(row.get("description")) else ""
            )

            entry = IncomeEntry(
                date=tx_date,
                symbol=symbol,
                amount=amount,
                currency=currency,
                amount_rsd=amount_rsd,
                exchange_rate=rate,
                income_type=income_type,
                description=description,
            )

            entries.append(entry)

        return entries, withholding_df

    def _group_entries_by_date_symbol_and_type(
        self,
        entries: list[IncomeEntry],
    ) -> dict[tuple[date, str, str], list[IncomeEntry]]:
        """
        Group income entries by date, symbol/currency, and income type.
        Each group will become a separate declaration.

        For dividends: groups by (date, symbol, income_type)
        For interest: groups by (date, currency_code, income_type)

        Args:
            entries: List of income entries.

        Returns:
            dict: {(date, symbol_or_currency, income_type): [entries]}
        """
        grouped = defaultdict(list)
        for entry in entries:
            # For interest: use currency code instead of symbol
            grouping_key = entry.currency.value if entry.income_type == "coupon" else entry.symbol

            key = (entry.date, grouping_key, entry.income_type)
            grouped[key].append(entry)

        return dict(grouped)

    def _calculate_tax_fields(
        self,
        group_entries: list[IncomeEntry],
        withholding_tax_rsd: Decimal,
    ) -> IncomeDeclarationEntry:
        """
        Calculate tax fields for a group of income entries.

        Args:
            group_entries: List of income entries for one declaration.
            withholding_tax_rsd: Foreign tax paid in RSD.

        Returns:
            IncomeDeclarationEntry with calculated tax fields.
        """
        # Calculate totals
        total_bruto = sum(entry.amount_rsd for entry in group_entries)
        # Round to 2 decimal places
        total_bruto = round(total_bruto, 2)

        # Tax rate: 15% for capital income
        tax_rate = Decimal("0.15")
        osnovica = Decimal(str(total_bruto))
        # Use quantize with ROUND_HALF_UP for taxes (standard rounding)
        obracunati_porez = (osnovica * tax_rate).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)

        # Round withholding tax
        porez_placen_drugoj_drzavi = Decimal(str(round(withholding_tax_rsd, 2)))

        # PorezZaUplatu = ObracunatiPorez - PorezPlacenDrugojDrzavi
        porez_za_uplatu = max(Decimal("0.00"), obracunati_porez - porez_placen_drugoj_drzavi)
        porez_za_uplatu = Decimal(str(round(porez_za_uplatu, 2)))

        # Get date and income_type from first entry (all should be same)
        first_entry = group_entries[0]

        # SifraVrstePrihoda:
        # 111402000 - Dividends from shares
        # 111403000 - Interest from bonds (coupons)
        sifra_vrste_prihoda = "111402000" if first_entry.income_type == "dividend" else "111403000"

        return IncomeDeclarationEntry(
            date=first_entry.date,
            sifra_vrste_prihoda=sifra_vrste_prihoda,
            bruto_prihod=Decimal(str(total_bruto)),
            osnovica_za_porez=osnovica,
            obracunati_porez=obracunati_porez,
            porez_placen_drugoj_drzavi=porez_placen_drugoj_drzavi,
            porez_za_uplatu=porez_za_uplatu,
        )

    def generate(
        self,
        start_date: date,
        end_date: date,
        filename: str | None = None,
        force: bool = False,
    ):
        """
        Generate PP OPO XML reports for income in period.

        Creates separate declarations for each day and income type
        (dividends and coupons are separate declarations even on same day).

        Args:
            start_date: Start date for the report period.
            end_date: End date for the report period.
            filename: Optional base filename. If not provided, will be generated.
            force: If True, create declaration with zero tax even if withholding tax not found.

        Yields:
            tuple[str, list[IncomeDeclarationEntry]]: (filename, entries) tuple.
                Each entry represents one declaration row with calculated tax fields.

        Raises:
            ValueError: If no transactions found, no income in period,
                or withholding tax not found (unless force=True).
        """
        # Get Transactions (DataFrame)
        df_transactions = self.storage.get_transactions()

        if df_transactions.empty:
            raise ValueError("No transactions found. Run `ibkr-porez get` first.")

        # Process income transactions and withholding taxes
        entries, withholding_df = self._process_income_transactions(
            df_transactions,
            start_date,
            end_date,
        )

        if not entries:
            raise ValueError("No income (dividends/coupons) found in this period.")

        # Group by date, symbol/currency, and income type (each group = one declaration)
        grouped = self._group_entries_by_date_symbol_and_type(entries)

        # Generate XML for each group
        for (declaration_date, _symbol_or_currency, income_type), group_entries in sorted(
            grouped.items(),
        ):
            # Get currency and symbol from first entry
            first_entry = group_entries[0]
            currency = first_entry.currency
            symbol = first_entry.symbol

            # Find withholding tax for this group (search in subsequent days)
            withholding_tax_rsd = self._find_withholding_tax(
                declaration_date,
                symbol,
                income_type,
                currency,
                group_entries,  # Pass group entries for entity_name/ISIN matching
                withholding_df,
                max_days_offset=7,
            )

            # Check if tax was found
            if withholding_tax_rsd == Decimal("0.00"):
                # Try to determine income description for error message
                if income_type == "coupon":
                    income_desc = f"{currency.value} {income_type} on {declaration_date}"
                else:
                    income_desc = f"{symbol} {income_type} on {declaration_date}"

                if not force:
                    raise ValueError(
                        f"Withholding tax in IBKR for payment {income_desc} not found. "
                        "To create a declaration with zero withholding tax, use the --force flag. "
                        "Warning: this means you will need to pay the tax in Serbia.",
                    )

            # Calculate tax fields for declaration entry
            declaration_entry = self._calculate_tax_fields(group_entries, withholding_tax_rsd)

            # Generate XML
            xml_content = self.xml_gen.generate_xml(
                group_entries,
                declaration_date,
                income_type,
                withholding_tax_rsd,
            )

            # Generate filename
            # Format: ppopo-{symbol}-{yyyy}-{mmdd}.xml
            # (matching sync style, without declaration number)
            # For interest: use currency code in filename; for dividends: use symbol
            filename_key = currency.value if income_type == "coupon" else symbol
            symbol_lower = filename_key.lower()
            date_str = declaration_date.strftime("%Y-%m%d")
            file_path = filename if filename else f"ppopo-{symbol_lower}-{date_str}.xml"

            # Write file
            with open(file_path, "w", encoding="utf-8") as f:
                f.write(xml_content)

            # Yield declaration entry (one per declaration)
            yield file_path, [declaration_entry]
