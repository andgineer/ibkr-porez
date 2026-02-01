"""ibkr-porez."""

import logging
import re
from collections import defaultdict
from datetime import datetime
from decimal import Decimal
from pathlib import Path

import pandas as pd
import rich_click as click
from pydantic import ValidationError
from rich.console import Console
from rich.logging import RichHandler
from rich.table import Table

from ibkr_porez import __version__
from ibkr_porez.config import UserConfig, config_manager
from ibkr_porez.ibkr_csv import CSVParser
from ibkr_porez.ibkr_flex_query import IBKRClient
from ibkr_porez.nbs import NBSClient
from ibkr_porez.report_gains import GainsReportGenerator
from ibkr_porez.report_income import IncomeReportGenerator
from ibkr_porez.report_params import ReportParams, ReportType
from ibkr_porez.storage import Storage
from ibkr_porez.tables import render_declaration_table
from ibkr_porez.tax import TaxCalculator
from ibkr_porez.validation import handle_validation_error

OUTPUT_FILE_DEFAULT = "output"

# Global Console instance to ensure logs and progress bars share the same stream
console = Console()


def _setup_logging_callback(ctx, param, value):  # noqa: ARG001
    if not value or ctx.resilient_parsing:
        return

    # Use RichHandler connected to the global console
    # rich_tracebacks=True gives nice coloured exceptions
    logging.basicConfig(
        level=logging.DEBUG,
        format="%(message)s",
        datefmt="[%X]",
        handlers=[RichHandler(console=console, rich_tracebacks=True)],
    )

    # Suppress chatty libraries
    logging.getLogger("urllib3").setLevel(logging.WARNING)
    logging.getLogger("requests").setLevel(logging.WARNING)


def verbose_option(f):
    return click.option(
        "--verbose",
        "-v",
        is_flag=True,
        expose_value=False,
        is_eager=True,
        callback=_setup_logging_callback,
        help="Enable verbose logging.",
    )(f)


@click.group(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/",
)
@click.version_option(version=__version__, prog_name="ibkr-porez")
def ibkr_porez() -> None:
    """Automated PPDG-3R tax reports for Interactive Brokers."""


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#configuration",
)
@verbose_option
def config():
    """Configure IBKR and personal details."""
    current_config = config_manager.load_config()

    console.print("[bold blue]Configuration Setup[/bold blue]")
    console.print(f"Config file location: {config_manager.config_path}\n")

    console.print(
        "[dim]Need help getting your IBKR Flex Token and Query ID? "
        "See [link=https://andgineer.github.io/ibkr-porez/ibkr/#flex-web-service]"
        "documentation[/link].[/dim]\n",
    )

    ibkr_token = click.prompt("IBKR Flex Token", default=current_config.ibkr_token)
    ibkr_query_id = click.prompt("IBKR Query ID", default=current_config.ibkr_query_id)

    personal_id = click.prompt("Personal Search ID (JMBG)", default=current_config.personal_id)
    full_name = click.prompt("Full Name", default=current_config.full_name)
    address = click.prompt("Address", default=current_config.address)
    city_code = click.prompt(
        "City/Municipality Code (Sifra opstine, e.g. 223 Novi Sad, 013 Novi Beograd. See portal)",
        default=current_config.city_code or "223",
    )
    phone = click.prompt("Phone Number", default=current_config.phone)
    email = click.prompt("Email", default=current_config.email)

    new_config = UserConfig(
        ibkr_token=ibkr_token,
        ibkr_query_id=ibkr_query_id,
        personal_id=personal_id,
        full_name=full_name,
        address=address,
        city_code=city_code,
        phone=phone,
        email=email,
    )

    config_manager.save_config(new_config)
    console.print("\n[bold green]Configuration saved successfully![/bold green]")


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#fetch-data-get",
)
@verbose_option
def get():
    """Sync data from IBKR and NBS."""
    cfg = config_manager.load_config()
    if not cfg.ibkr_token or not cfg.ibkr_query_id:
        console.print("[red]Missing Configuration! Run `ibkr-porez config` first.[/red]")
        return

    storage = Storage()
    ibkr = IBKRClient(cfg.ibkr_token, cfg.ibkr_query_id)
    nbs = NBSClient(storage)

    with console.status("[bold green]Fetching data from IBKR...[/bold green]"):
        try:
            # 1. Fetch XML
            console.print("[blue]Fetching full report...[/blue]")
            xml_content = ibkr.fetch_latest_report()

            # Save raw backup
            import time

            filename = f"flex_report_{int(time.time())}.xml"
            storage.save_raw_report(xml_content, filename)

            # 2. Parse
            transactions = ibkr.parse_report(xml_content)

            # 3. Save
            count_inserted, count_updated = storage.save_transactions(transactions)
            msg = f"Fetched {len(transactions)} transactions."
            stats = f"({count_inserted} new, {count_updated} updated)"
            console.print(f"[green]{msg} {stats}[/green]")

        except Exception as e:  # noqa: BLE001
            # Stop if XML fetch/parse fails
            console.print(f"[bold red]Error:[/bold red] {e}")
            console.print_exception()
            return

    # 4. Sync Rates (Priming Cache) - OUTSIDE status context
    try:
        console.print("[blue]Syncing NBS exchange rates...[/blue]")
        dates_to_fetch = set()
        for tx in transactions:
            dates_to_fetch.add((tx.date, tx.currency))
            if tx.open_date:
                dates_to_fetch.add((tx.open_date, tx.currency))

        from rich.progress import track

        for d, curr in track(dates_to_fetch, description="Fetching rates...", console=console):
            nbs.get_rate(d, curr)

        console.print("[bold green]Sync Complete![/bold green]")

    except Exception as e:  # noqa: BLE001
        console.print(f"[bold red]Rate Sync Error:[/bold red] {e}")
        console.print_exception()


@ibkr_porez.command(
    "import",
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#import-historical-data-import",
)
@click.argument("file_path", type=click.Path(exists=True, path_type=Path))
@verbose_option
def import_file(file_path: Path):
    """Import historical transactions from CSV Activity Statement."""
    storage = Storage()
    nbs = NBSClient(storage)

    console.print(f"[blue]Importing from {file_path}...[/blue]")

    try:
        parser = CSVParser()
        with open(file_path, encoding="utf-8-sig") as f:
            transactions = parser.parse(f)

        if not transactions:
            console.print("[yellow]No valid transactions found in file.[/yellow]")
            return

        count_inserted, count_updated = storage.save_transactions(transactions)
        msg = f"Parsed {len(transactions)} transactions."
        stats = f"({count_inserted} new, {count_updated} updated)"
        console.print(f"[green]{msg} {stats}[/green]")

        # Sync Rates
        console.print("[blue]Syncing NBS exchange rates for imported data...[/blue]")
        dates_to_fetch = set()
        for tx in transactions:
            dates_to_fetch.add((tx.date, tx.currency))
            if tx.open_date:
                dates_to_fetch.add((tx.open_date, tx.currency))

        from rich.progress import track

        for d, curr in track(dates_to_fetch, description="Fetching rates...", console=console):
            nbs.get_rate(d, curr)

        console.print("[bold green]Import Complete![/bold green]")

    except Exception as e:  # noqa: BLE001
        console.print(f"[bold red]Import Failed:[/bold red] {e}")
        console.print_exception()


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#show-statistics-show",
)
@click.option("--year", type=int, help="Filter by year (e.g. 2026)")
@click.option("-t", "--ticker", type=str, help="Show detailed breakdown for specific ticker")
@click.option(
    "-m",
    "--month",
    type=str,
    help="Show detailed breakdown for specific month (YYYY-MM, YYYYMM, or MM)",
)
@verbose_option
def show(year: int | None, ticker: str | None, month: str | None):  # noqa: C901,PLR0912,PLR0915
    """Show tax report (Sales only)."""
    storage = Storage()
    nbs = NBSClient(storage)
    tax_calc = TaxCalculator(nbs)

    # Load transactions
    # Note: We must load ALL transactions to ensure FIFO context is correct.
    # We will filter for display later.
    df_transactions = storage.get_transactions()

    if df_transactions.empty:
        console.print("[yellow]No transactions found. Run `ibkr-porez get`.[/yellow]")
        return

    # Process Taxable Sales (FIFO)
    sales_entries = tax_calc.process_trades(df_transactions)

    target_year = year
    target_month = None

    # Parse Month Argument if provided
    if month:
        # Validate format
        # 1. YYYY-MM
        m_dash = re.match(r"^(\d{4})-(\d{1,2})$", month)
        # 2. YYYYMM
        m_compact = re.match(r"^(\d{4})(\d{2})$", month)
        # 3. MM or M
        m_only = re.match(r"^(\d{1,2})$", month)

        if m_dash:
            target_year = int(m_dash.group(1))
            target_month = int(m_dash.group(2))
        elif m_compact:
            target_year = int(m_compact.group(1))
            target_month = int(m_compact.group(2))
        elif m_only:
            target_month = int(m_only.group(1))
            if not target_year:
                # Find latest year with data for this month
                years_with_data = set()
                for e in sales_entries:
                    if e.sale_date.month == target_month:
                        years_with_data.add(e.sale_date.year)

                # Also check dividends?
                # Ideally yes, but let's stick to sales for the detailed view context
                # or generally present data.
                # Let's check dividends too.
                if "type" in df_transactions.columns:
                    divs_check = df_transactions[df_transactions["type"] == "DIVIDEND"]
                    for d in pd.to_datetime(divs_check["date"]).dt.date:
                        if d.month == target_month:
                            years_with_data.add(d.year)

                # Default to current year if no data found
                target_year = max(years_with_data) if years_with_data else datetime.now().year
        else:
            console.print(f"[red]Invalid month format: {month}. Use YYYY-MM, YYYYMM, or MM.[/red]")
            return

    # Determine Mode: Detailed List vs Monthly Summary
    # If a TICKER is specified, we almost certainly want the Detailed List of executions.
    # If only Month is specified, user might want a Monthly Summary (filtered), OR detailed list.
    # User feedback suggests they want "detailed calculation" when they specify ticker/month.

    show_detailed_list = False
    if ticker:
        show_detailed_list = True

    # If detailed list is requested:
    if show_detailed_list:
        # Filter entries
        filtered_entries = []
        for e in sales_entries:
            if ticker and e.ticker != ticker:
                continue
            if target_year and e.sale_date.year != target_year:
                continue
            if target_month and e.sale_date.month != target_month:
                continue
            filtered_entries.append(e)

        if not filtered_entries:
            msg = "[yellow]No sales found matching criteria"
            if ticker:
                msg += f" ticker={ticker}"
            if target_year:
                msg += f" year={target_year}"
            if target_month:
                msg += f" month={target_month}"
            msg += "[/yellow]"
            console.print(msg)
            return

        title_parts = []
        if ticker:
            title_parts.append(ticker)
        if target_year:
            if target_month:
                title_parts.append(f"{target_year}-{target_month:02d}")
            else:
                title_parts.append(str(target_year))

        table_title = f"Detailed Report: {' - '.join(title_parts)}"
        table = Table(title=table_title, box=None)  # Cleaner look

        table.add_column("Sale Date", justify="left")
        table.add_column("Qty", justify="right")
        table.add_column("Sale Price", justify="right")
        table.add_column("Sale Rate", justify="right")
        table.add_column("Sale Val (RSD)", justify="right")  # ADDED

        table.add_column("Buy Date", justify="left")
        table.add_column("Buy Price", justify="right")
        table.add_column("Buy Rate", justify="right")
        table.add_column("Buy Val (RSD)", justify="right")  # ADDED

        table.add_column("Gain (RSD)", justify="right")

        total_pnl = Decimal(0)

        for e in filtered_entries:
            total_pnl += e.capital_gain_rsd
            table.add_row(
                str(e.sale_date),
                f"{e.quantity:.2f}",
                f"{e.sale_price:.2f}",
                f"{e.sale_exchange_rate:.4f}",
                f"{e.sale_value_rsd:,.0f}",  # No decimals for large RSD values usually cleaner
                str(e.purchase_date),
                f"{e.purchase_price:.2f}",
                f"{e.purchase_exchange_rate:.4f}",
                f"{e.purchase_value_rsd:,.0f}",
                f"[bold]{e.capital_gain_rsd:,.2f}[/bold]",
            )

        console.print(table)
        console.print(f"[bold]Total P/L: {total_pnl:,.2f} RSD[/bold]")
        return

    # Fallback to Aggregated View (Summary)
    # Group by Month-Year and Ticker
    # Structure: { "YYYY-MM": { "TICKER": { "divs": 0.0, "sales_count": 0, "pnl": Decimal(0) } } }
    stats = defaultdict(
        lambda: defaultdict(lambda: {"divs": Decimal(0), "sales_count": 0, "pnl": Decimal(0)}),
    )

    for entry in sales_entries:  # Already filtered by year (if --year passed, but maybe not by -m)
        if target_year and entry.sale_date.year != target_year:
            continue
        if target_month and entry.sale_date.month != target_month:
            continue
        if (
            ticker and entry.ticker != ticker
        ):  # Should be handled by Detail view usually, but keeping logic safely
            continue

        month_key = entry.sale_date.strftime("%Y-%m")
        t = entry.ticker
        stats[month_key][t]["sales_count"] += 1
        stats[month_key][t]["pnl"] += entry.capital_gain_rsd

    # Process Dividends
    if "type" in df_transactions.columns:
        divs = df_transactions[df_transactions["type"] == "DIVIDEND"].copy()

        for _, row in divs.iterrows():
            d = row["date"]  # date object
            if target_year and d.year != target_year:
                continue
            if target_month and d.month != target_month:
                continue

            t = row["symbol"]
            if ticker and t != ticker:
                continue

            curr = row["currency"]
            amt = Decimal(str(row["amount"]))

            # Rate
            from ibkr_porez.models import Currency

            try:
                c_enum = Currency(curr)
                rate = nbs.get_rate(d, c_enum)
                if rate:
                    val = amt * rate
                    month_key = d.strftime("%Y-%m")
                    stats[month_key][t]["divs"] += val
            except ValueError:
                pass

    # Print Table
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

    console.print(table)


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#generate-capital-gains-tax-report-report",
)
@click.option(
    "--type",
    type=click.Choice(["gains", "income"], case_sensitive=False),
    default="gains",
    help="Report type: 'gains' for PPDG-3R (capital gains) or 'income' for PP OPO (capital income)",
)
@click.option(
    "--half",
    required=False,
    help=(
        "Half-year period (e.g. 2026-1, 20261). "
        "For --type=gains, defaults to the last complete half-year if not provided."
    ),
)
@click.option(
    "--from",
    "from_date",
    required=False,
    help=(
        "Start date (YYYY-MM-DD). "
        "If --from and --to are not provided, they default to current month "
        "(from 1st to today). If only --from is provided, --to defaults to --from."
    ),
)
@click.option(
    "--to",
    "to_date",
    required=False,
    help=(
        "End date (YYYY-MM-DD). "
        "If --from and --to are not provided, they default to current month "
        "(from 1st to today). If only --from is provided, --to defaults to --from."
    ),
)
@verbose_option
def report(  # noqa: C901,PLR0915
    type: str,
    half: str | None,
    from_date: str | None,
    to_date: str | None,
):  # noqa: PLR0913
    """Generate tax reports (PPDG-3R for capital gains or PP OPO for capital income)."""
    try:
        params = ReportParams.model_validate(
            {
                "type": type,
                "half": half,
                "from": from_date,
                "to": to_date,
            },
        )
        start_date_obj, end_date_obj = params.get_period()
    except ValidationError as e:
        handle_validation_error(e, console)
        return
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        return

    if params.type == ReportType.GAINS:
        console.print(
            f"[bold blue]Generating PPDG-3R Report for "
            f"({start_date_obj} to {end_date_obj})[/bold blue]",
        )

        try:
            generator = GainsReportGenerator()
            # Generate filename from half if available
            filename = None
            if params.half:
                half_match = re.match(r"^(\d{4})-(\d)$", params.half) or re.match(
                    r"^(\d{4})(\d)$",
                    params.half,
                )
                if half_match:
                    target_year = int(half_match.group(1))
                    target_half = int(half_match.group(2))
                    filename = f"ppdg3r_{target_year}_H{target_half}.xml"

            filename, entries = generator.generate(
                start_date=start_date_obj,
                end_date=end_date_obj,
                filename=filename,
            )

            console.print(f"[bold green]Report generated: {filename}[/bold green]")
            console.print(f"Total Entries: {len(entries)}")

            table = render_declaration_table(entries)
            console.print(table)
            console.print(
                "[dim]Use these values to cross-check with the portal "
                "or fill manually if needed.[/dim]",
            )

            console.print("\n[bold red]ATTENTION: Step 8 (Upload)[/bold red]")
            console.print(
                "[bold]You MUST manually upload your IBKR Activity Report (PDF) "
                "in 'Deo 8' on the ePorezi portal. "
                "See [link=https://andgineer.github.io/ibkr-porez/ibkr/#export-full-history-for-import-command]"
                "Export Full History[/link].[/bold]",
            )

        except ValueError as e:
            console.print(f"[yellow]{e}[/yellow]")
            return

    elif params.type == ReportType.INCOME:
        try:
            generator = IncomeReportGenerator()
            generator.generate(
                start_date=start_date_obj,
                end_date=end_date_obj,
            )
        except NotImplementedError as e:
            console.print(f"[yellow]{e}[/yellow]")
            console.print(
                f"[dim]Requested period: {start_date_obj} to {end_date_obj}[/dim]",
            )
            return


if __name__ == "__main__":  # pragma: no cover
    ibkr_porez()  # pylint: disable=no-value-for-parameter
