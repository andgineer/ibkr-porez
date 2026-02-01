"""Generator for PP OPO (Capital Income) reports."""

from datetime import date

from ibkr_porez.storage import Storage


class IncomeReportGenerator:
    """Generator for PP OPO (Capital Income) reports."""

    def __init__(self):
        self.storage = Storage()

    def generate(
        self,
        start_date: date,
        end_date: date,
        filename: str | None = None,
    ) -> str:
        """
        Generate PP OPO XML report.

        Args:
            start_date: Start date for the report period.
            end_date: End date for the report period.
            filename: Optional filename. If not provided, will be generated.

        Returns:
            str: Filename of generated report.

        Raises:
            NotImplementedError: This feature is not yet implemented.
        """
        raise NotImplementedError(
            "PP OPO report generation is not yet implemented. "
            "This feature will generate PP OPO declarations for capital income "
            "(dividends and coupons).",
        )
