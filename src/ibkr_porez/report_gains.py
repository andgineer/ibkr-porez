"""Generator for PPDG-3R (Capital Gains) reports."""

from datetime import date

from ibkr_porez.config import config_manager
from ibkr_porez.declaration_gains_xml import XMLGenerator
from ibkr_porez.models import TaxReportEntry
from ibkr_porez.nbs import NBSClient
from ibkr_porez.storage import Storage
from ibkr_porez.tax import TaxCalculator


class GainsReportGenerator:
    """Generator for PPDG-3R (Capital Gains) reports."""

    def __init__(self):
        self.cfg = config_manager.load_config()
        self.storage = Storage()
        self.nbs = NBSClient(self.storage)
        self.tax_calc = TaxCalculator(self.nbs)
        self.xml_gen = XMLGenerator(self.cfg)

    def generate(
        self,
        start_date: date,
        end_date: date,
        filename: str | None = None,
    ) -> list[tuple[str, list[TaxReportEntry]]]:
        """
        Generate PPDG-3R XML report.

        Args:
            start_date: Start date for the report period.
            end_date: End date for the report period.
            filename: Optional filename. If not provided, will be generated.

        Returns:
            list[tuple[str, list[TaxReportEntry]]]: List of (filename, entries) tuples.
                For gains, this will always be a single-element list.

        Raises:
            ValueError: If no transactions found or no taxable sales in period.
        """
        # Get Transactions (DataFrame)
        # Load ALL to ensure FIFO context
        df_transactions = self.storage.get_transactions()

        if df_transactions.empty:
            raise ValueError("No transactions found. Run `ibkr-porez get` first.")

        # Process FIFO for all
        all_entries = self.tax_calc.process_trades(df_transactions)

        # Filter for Period
        entries = []
        for e in all_entries:
            if start_date <= e.sale_date <= end_date:
                entries.append(e)

        if not entries:
            raise ValueError("No taxable sales found in this period.")

        # Generate XML
        xml_content = self.xml_gen.generate_xml(entries, start_date, end_date)

        # Generate filename if not provided
        if filename is None:
            filename = f"ppdg3r_{start_date}_{end_date}.xml"

        # Write file
        with open(filename, "w", encoding="utf-8") as f:
            f.write(xml_content)

        return [(filename, entries)]
