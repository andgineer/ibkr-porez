"""Generator for PPDG-3R (Capital Gains) reports."""

from datetime import date

from ibkr_porez.declaration_gains_xml import XMLGenerator
from ibkr_porez.report_base import ReportGeneratorBase
from ibkr_porez.tax import TaxCalculator


class GainsReportGenerator(ReportGeneratorBase):
    """Generator for PPDG-3R (Capital Gains) reports."""

    JUNE_MONTH = 6

    def __init__(self):
        super().__init__()
        self.tax_calc = TaxCalculator(self.nbs)
        self.xml_gen = XMLGenerator(self.cfg)

    def filename(self, end_date: date) -> str:  # type: ignore[override]
        """
        Generate filename for gains report.

        Args:
            end_date: End date for the report period.

        Returns:
            Filename string in format: ppdg3r-yyyy-Hh.xml
        """
        year = end_date.year
        half = 1 if end_date.month <= self.JUNE_MONTH else 2
        return f"ppdg3r-{year}-H{half}.xml"

    def generate(
        self,
        start_date: date,
        end_date: date,
        force: bool = False,  # noqa: ARG002
    ):
        """
        Generate PPDG-3R XML report.

        Args:
            start_date: Start date for the report period.
            end_date: End date for the report period.

        Yields:
            tuple[str, str, list[TaxReportEntry]]: (filename, xml_content, entries) tuple.
                For gains, this will always be a single yield.

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

        xml_content = self.xml_gen.generate_xml(entries, start_date, end_date)
        filename = self.filename(end_date)
        yield filename, xml_content, entries
