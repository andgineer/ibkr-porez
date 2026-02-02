"""Generator for PP OPO (Capital Income) reports."""

from collections import defaultdict
from datetime import date
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

    def _process_income_transactions(  # noqa: C901
        self,
        df_transactions: pd.DataFrame,
        start_date: date,
        end_date: date,
    ) -> tuple[list[IncomeEntry], dict[tuple[date, str, str], Decimal]]:
        """
        Process DIVIDEND and INTEREST transactions and convert to IncomeEntry.
        Also collect WITHHOLDING_TAX for foreign tax calculation.

        Args:
            df_transactions: DataFrame with all transactions.
            start_date: Start date for filtering.
            end_date: End date for filtering.

        Returns:
            tuple: (list[IncomeEntry], dict[(date, symbol, income_type): withholding_tax_rsd])
        """
        if df_transactions.empty:
            return [], {}

        # Filter for DIVIDEND and INTEREST in period
        income_types = [TransactionType.DIVIDEND.value, TransactionType.INTEREST.value]
        income_df = df_transactions[
            (df_transactions["type"].isin(income_types))
            & (pd.to_datetime(df_transactions["date"]).dt.date >= start_date)
            & (pd.to_datetime(df_transactions["date"]).dt.date <= end_date)
        ].copy()

        if income_df.empty:
            return [], {}

        # Also get WITHHOLDING_TAX for foreign tax calculation
        withholding_df = df_transactions[
            (df_transactions["type"] == TransactionType.WITHHOLDING_TAX.value)
            & (pd.to_datetime(df_transactions["date"]).dt.date >= start_date)
            & (pd.to_datetime(df_transactions["date"]).dt.date <= end_date)
        ].copy()

        # Build withholding tax map: (date, symbol, income_type) -> tax_rsd
        withholding_map: dict[tuple[date, str, str], Decimal] = {}

        for _, row in withholding_df.iterrows():
            tx_date = pd.to_datetime(row["date"]).date()
            symbol = str(row["symbol"]) if pd.notna(row["symbol"]) else "UNKNOWN"
            amount = Decimal(str(row["amount"]))
            currency_str = str(row["currency"])

            # WITHHOLDING_TAX is negative, we need absolute value
            tax_amount = abs(amount)

            try:
                currency = Currency(currency_str)
            except ValueError:
                continue

            rate = self.nbs.get_rate(tx_date, currency)
            if not rate:
                continue

            tax_rsd = tax_amount * rate

            # Determine income type from description or assume dividend
            # WITHHOLDING_TAX usually matches DIVIDEND transactions
            income_type = "dividend"  # Default, could be improved

            key = (tx_date, symbol, income_type)
            withholding_map[key] = withholding_map.get(key, Decimal("0.00")) + tax_rsd

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

        return entries, withholding_map

    def _group_entries_by_date_symbol_and_type(
        self,
        entries: list[IncomeEntry],
    ) -> dict[tuple[date, str, str], list[IncomeEntry]]:
        """
        Group income entries by date, symbol, and income type.
        Each group will become a separate declaration.

        Args:
            entries: List of income entries.

        Returns:
            dict: {(date, symbol, income_type): [entries]}
        """
        grouped = defaultdict(list)
        for entry in entries:
            key = (entry.date, entry.symbol, entry.income_type)
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
    ):
        """
        Generate PP OPO XML reports for income in period.

        Creates separate declarations for each day and income type
        (dividends and coupons are separate declarations even on same day).

        Args:
            start_date: Start date for the report period.
            end_date: End date for the report period.
            filename: Optional base filename. If not provided, will be generated.

        Yields:
            tuple[str, list[IncomeDeclarationEntry]]: (filename, entries) tuple.
                Each entry represents one declaration row with calculated tax fields.

        Raises:
            ValueError: If no transactions found or no income in period.
        """
        # Get Transactions (DataFrame)
        df_transactions = self.storage.get_transactions()

        if df_transactions.empty:
            raise ValueError("No transactions found. Run `ibkr-porez get` first.")

        # Process income transactions and withholding taxes
        entries, withholding_map = self._process_income_transactions(
            df_transactions,
            start_date,
            end_date,
        )

        if not entries:
            raise ValueError("No income (dividends/coupons) found in this period.")

        # Group by date, symbol, and income type (each group = one declaration)
        grouped = self._group_entries_by_date_symbol_and_type(entries)

        # Generate XML for each group
        for (declaration_date, symbol, income_type), group_entries in sorted(grouped.items()):
            # Get withholding tax for this group
            withholding_key = (declaration_date, symbol, income_type)
            withholding_tax_rsd = withholding_map.get(withholding_key, Decimal("0.00"))

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
            if filename:
                # If base filename provided, append date, symbol and type
                base = filename.replace(".xml", "")
                file_path = f"{base}_{declaration_date}_{symbol}_{income_type}.xml"
            else:
                file_path = f"ppopo_{declaration_date}_{symbol}_{income_type}.xml"

            # Write file
            with open(file_path, "w", encoding="utf-8") as f:
                f.write(xml_content)

            # Yield declaration entry (one per declaration)
            yield file_path, [declaration_entry]
