"""Controller for show command."""

import re
from collections import defaultdict
from datetime import datetime
from decimal import Decimal

import pandas as pd
from rich.table import Table

from ibkr_porez.models import Currency, TaxReportEntry
from ibkr_porez.nbs import NBSClient
from ibkr_porez.storage import Storage
from ibkr_porez.tax import TaxCalculator


class ShowStatistics:
    def __init__(self):
        self.storage = Storage()
        self.nbs = NBSClient(self.storage)
        self.tax_calc = TaxCalculator(self.nbs)

    def parse_month(self, month: str | None) -> tuple[int | None, int | None]:
        """
        Parse month argument.

        Args:
            month: Month string in format YYYY-MM, YYYYMM, or MM.

        Returns:
            tuple[int | None, int | None]: (target_year, target_month)

        Raises:
            ValueError: If month format is invalid.
        """
        if not month:
            return None, None

        m_dash = re.match(r"^(\d{4})-(\d{1,2})$", month)
        m_compact = re.match(r"^(\d{4})(\d{2})$", month)
        m_only = re.match(r"^(\d{1,2})$", month)

        if m_dash:
            return int(m_dash.group(1)), int(m_dash.group(2))
        if m_compact:
            return int(m_compact.group(1)), int(m_compact.group(2))
        if m_only:
            return None, int(m_only.group(1))

        raise ValueError(f"Invalid month format: {month}. Use YYYY-MM, YYYYMM, or MM.")

    def resolve_month_year(
        self,
        month: int | None,
        year: int | None,
        sales_entries: list[TaxReportEntry],
        df_transactions: pd.DataFrame,
    ) -> tuple[int, int]:
        """
        Resolve target year and month from parameters.

        If only month is provided, finds the latest year with data for that month.

        Args:
            month: Target month (1-12) or None.
            year: Target year or None.
            sales_entries: List of sales entries to search for data.
            df_transactions: DataFrame with all transactions.

        Returns:
            tuple[int, int]: (target_year, target_month)
        """
        if month and not year:
            # Find latest year with data for this month
            years_with_data = set()
            for e in sales_entries:
                if e.sale_date.month == month:
                    years_with_data.add(e.sale_date.year)

            # Also check dividends
            if "type" in df_transactions.columns:
                divs_check = df_transactions[df_transactions["type"] == "DIVIDEND"]
                for d in pd.to_datetime(divs_check["date"]).dt.date:
                    if d.month == month:
                        years_with_data.add(d.year)

            # Default to current year if no data found
            year = max(years_with_data) if years_with_data else datetime.now().year

        return year or datetime.now().year, month or datetime.now().month

    def get_detailed_entries(
        self,
        sales_entries: list[TaxReportEntry],
        ticker: str | None,
        year: int | None,
        month: int | None,
    ) -> list[TaxReportEntry]:
        """
        Filter sales entries for detailed view.

        Args:
            sales_entries: List of all sales entries.
            ticker: Filter by ticker (optional).
            year: Filter by year (optional).
            month: Filter by month (optional).

        Returns:
            list[TaxReportEntry]: Filtered entries.
        """
        filtered_entries = []
        for e in sales_entries:
            if ticker and e.ticker != ticker:
                continue
            if year and e.sale_date.year != year:
                continue
            if month and e.sale_date.month != month:
                continue
            filtered_entries.append(e)

        return filtered_entries

    def create_detailed_table(
        self,
        entries: list[TaxReportEntry],
        ticker: str | None,
        year: int | None,
        month: int | None,
    ) -> tuple[Table, Decimal]:
        """
        Create detailed table for sales entries.

        Args:
            entries: List of sales entries to display.
            ticker: Ticker filter (for title).
            year: Year filter (for title).
            month: Month filter (for title).

        Returns:
            tuple[Table, Decimal]: (table, total_pnl)
        """
        title_parts = []
        if ticker:
            title_parts.append(ticker)
        if year:
            if month:
                title_parts.append(f"{year}-{month:02d}")
            else:
                title_parts.append(str(year))

        table_title = f"Detailed Report: {' - '.join(title_parts)}"
        table = Table(title=table_title, box=None)

        table.add_column("Sale Date", justify="left")
        table.add_column("Qty", justify="right")
        table.add_column("Sale Price", justify="right")
        table.add_column("Sale Rate", justify="right")
        table.add_column("Sale Val (RSD)", justify="right")
        table.add_column("Buy Date", justify="left")
        table.add_column("Buy Price", justify="right")
        table.add_column("Buy Rate", justify="right")
        table.add_column("Buy Val (RSD)", justify="right")
        table.add_column("Gain (RSD)", justify="right")

        total_pnl = Decimal(0)

        for e in entries:
            total_pnl += e.capital_gain_rsd
            table.add_row(
                str(e.sale_date),
                f"{e.quantity:.2f}",
                f"{e.sale_price:.2f}",
                f"{e.sale_exchange_rate:.4f}",
                f"{e.sale_value_rsd:,.0f}",
                str(e.purchase_date),
                f"{e.purchase_price:.2f}",
                f"{e.purchase_exchange_rate:.4f}",
                f"{e.purchase_value_rsd:,.0f}",
                f"[bold]{e.capital_gain_rsd:,.2f}[/bold]",
            )

        return table, total_pnl

    def _process_sales_for_stats(
        self,
        sales_entries: list[TaxReportEntry],
        stats: dict,
        ticker: str | None,
        year: int | None,
        month: int | None,
    ) -> None:
        """Process sales entries and add to stats."""
        for entry in sales_entries:
            if year and entry.sale_date.year != year:
                continue
            if month and entry.sale_date.month != month:
                continue
            if ticker and entry.ticker != ticker:
                continue

            month_key = entry.sale_date.strftime("%Y-%m")
            t = entry.ticker
            stats[month_key][t]["sales_count"] += 1
            stats[month_key][t]["pnl"] += entry.capital_gain_rsd

    def _process_dividends_for_stats(
        self,
        df_transactions: pd.DataFrame,
        stats: dict,
        ticker: str | None,
        year: int | None,
        month: int | None,
    ) -> None:
        """Process dividends and add to stats."""
        if "type" not in df_transactions.columns:
            return

        divs = df_transactions[df_transactions["type"] == "DIVIDEND"].copy()

        for _, row in divs.iterrows():
            d = row["date"]
            if year and d.year != year:
                continue
            if month and d.month != month:
                continue

            t = row["symbol"]
            if ticker and t != ticker:
                continue

            curr = row["currency"]
            amt = Decimal(str(row["amount"]))

            try:
                c_enum = Currency(curr)
                rate = self.nbs.get_rate(d, c_enum)
                if rate:
                    val = amt * rate
                    month_key = d.strftime("%Y-%m")
                    stats[month_key][t]["divs"] += val
            except ValueError:
                pass

    def get_aggregated_stats(
        self,
        sales_entries: list[TaxReportEntry],
        df_transactions: pd.DataFrame,
        ticker: str | None,
        year: int | None,
        month: int | None,
    ) -> dict[str, dict[str, dict[str, Decimal | int]]]:
        """
        Calculate aggregated statistics by month and ticker.

        Args:
            sales_entries: List of all sales entries.
            df_transactions: DataFrame with all transactions.
            ticker: Filter by ticker (optional).
            year: Filter by year (optional).
            month: Filter by month (optional).

        Returns:
            dict: Nested dict structure:
            { "YYYY-MM": { "TICKER": { "divs": Decimal, "sales_count": int, "pnl": Decimal } } }
        """
        stats = defaultdict(
            lambda: defaultdict(lambda: {"divs": Decimal(0), "sales_count": 0, "pnl": Decimal(0)}),
        )

        self._process_sales_for_stats(sales_entries, stats, ticker, year, month)
        self._process_dividends_for_stats(df_transactions, stats, ticker, year, month)

        return dict(stats)

    def create_aggregated_table(
        self,
        stats: dict[str, dict[str, dict[str, Decimal | int]]],
    ) -> Table:
        """
        Create aggregated monthly report table.

        Args:
            stats: Statistics dictionary from get_aggregated_stats.

        Returns:
            Table: Formatted table.
        """
        table = Table(title="Monthly Report Breakdown")
        table.add_column("Month", justify="left")
        table.add_column("Ticker", justify="left")
        table.add_column("Dividends (RSD)", justify="right")
        table.add_column("Sales Count", justify="right")
        table.add_column("Realized P/L (RSD)", justify="right")

        rows = []
        for m, tickers in stats.items():
            for t, data in tickers.items():
                rows.append((m, t, data))

        rows.sort(key=lambda x: x[1])  # Ticker ASC
        rows.sort(key=lambda x: x[0], reverse=True)  # Month DESC

        current_month: str | None = None
        for m, t, data in rows:
            if current_month != m:
                table.add_section()
                current_month = m

            table.add_row(
                m,
                t,
                f"{data['divs']:,.2f}",
                str(data["sales_count"]),
                f"{data['pnl']:,.2f}",
            )

        return table

    def generate(
        self,
        year: int | None = None,
        ticker: str | None = None,
        month: str | None = None,
    ) -> tuple[Table | None, Decimal | None, str | None]:
        """
        Generate statistics display.

        Args:
            year: Filter by year (optional).
            ticker: Filter by ticker (optional).
            month: Filter by month in format YYYY-MM, YYYYMM, or MM (optional).

        Returns:
            tuple[Table | None, Decimal | None, str | None]: (table, total_pnl, error_message)
            - If detailed view: returns (table, total_pnl, None)
            - If aggregated view: returns (table, None, None)
            - If error: returns (None, None, error_message)
            - If no data: returns (None, None, error_message)

        Raises:
            ValueError: If month format is invalid.
        """
        # Load transactions
        df_transactions = self.storage.get_transactions()

        if df_transactions.empty:
            return None, None, "No transactions found. Run `ibkr-porez get`."

        # Process Taxable Sales (FIFO)
        sales_entries = self.tax_calc.process_trades(df_transactions)

        # Parse month
        parsed_year, parsed_month = self.parse_month(month)
        target_year = year or parsed_year
        target_month = parsed_month

        # Resolve year if only month provided (but not year)
        if target_month and not target_year:
            target_year, target_month = self.resolve_month_year(
                target_month,
                None,
                sales_entries,
                df_transactions,
            )

        # Determine mode: detailed list if ticker is specified
        show_detailed_list = bool(ticker)

        if show_detailed_list:
            # Detailed view
            filtered_entries = self.get_detailed_entries(
                sales_entries,
                ticker,
                target_year,
                target_month,
            )

            if not filtered_entries:
                msg_parts = ["No sales found matching criteria"]
                if ticker:
                    msg_parts.append(f"ticker={ticker}")
                if target_year:
                    msg_parts.append(f"year={target_year}")
                if target_month:
                    msg_parts.append(f"month={target_month}")
                return None, None, " ".join(msg_parts)

            table, total_pnl = self.create_detailed_table(
                filtered_entries,
                ticker,
                target_year,
                target_month,
            )
            return table, total_pnl, None

        # Aggregated view
        stats = self.get_aggregated_stats(
            sales_entries,
            df_transactions,
            ticker,
            target_year,
            target_month,
        )

        table = self.create_aggregated_table(stats)
        return table, None, None
