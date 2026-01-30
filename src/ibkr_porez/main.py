"""ibkr-porez."""

from datetime import date
from pathlib import Path

import rich_click as click

from ibkr_porez import __version__
from ibkr_porez.config import UserConfig, config_manager
from ibkr_porez.ibkr import IBKRClient
from ibkr_porez.nbs import NBSClient
from ibkr_porez.storage import Storage
from ibkr_porez.tax import TaxCalculator

# click.rich_click.USE_MARKDOWN = True
OUTPUT_FILE_DEFAULT = "output"


@click.group()
@click.version_option(version=__version__, prog_name="ibkr-porez")
def ibkr_porez() -> None:
    """
    Automated PPDG-3R tax reports for Interactive Brokers.
    """


@ibkr_porez.command()
def config():
    """Configure IBKR and personal details."""
    current_config = config_manager.load_config()

    console = click.get_current_context().find_root().info_name  # type: ignore
    from rich.console import Console

    console = Console()

    console.print("[bold blue]Configuration Setup[/bold blue]")
    console.print(f"Config file location: {config_manager.config_path}\n")

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


@ibkr_porez.command()
@click.option("--force", "-f", is_flag=True, help="Force full fetch (ignore local history).")
def get(force: bool):
    """Sync data from IBKR and NBS."""
    from rich.console import Console

    console = Console()

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
            last_date = None
            if not force:
                last_date = storage.get_last_transaction_date()

            if last_date:
                console.print(
                    f"[blue]Found existing data up to {last_date}. Fetching updates...[/blue]",
                )
                # We start from the last date to catch any corrections on that day
                xml_content = ibkr.fetch_latest_report(start_date=last_date)
            else:
                msg = "Fetching full report (last 365 days)..."
                if force:
                    msg = "Force update enabled. " + msg
                console.print(f"[blue]{msg}[/blue]")
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

            # 4. Sync Rates (Priming Cache)
            console.print("[blue]Syncing NBS exchange rates...[/blue]")
            dates_to_fetch = set()
            for tx in transactions:
                dates_to_fetch.add((tx.date, tx.currency))
                if tx.open_date:
                    dates_to_fetch.add((tx.open_date, tx.currency))

            from rich.progress import track

            for d, curr in track(dates_to_fetch, description="Fetching rates..."):
                nbs.get_rate(d, curr)

            console.print("[bold green]Sync Complete![/bold green]")

        except Exception as e:  # noqa: BLE001
            console.print(f"[bold red]Error:[/bold red] {e}")
            import traceback

            traceback.print_exc()


@ibkr_porez.command("import")
@click.argument("file_path", type=click.Path(exists=True, path_type=Path))
def import_file(file_path: Path):
    """Import historical transactions from CSV Activity Statement."""
    from rich.console import Console

    from ibkr_porez.parsers.csv_parser import CSVParser

    console = Console()
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

        for d, curr in track(dates_to_fetch, description="Fetching rates..."):
            nbs.get_rate(d, curr)

        console.print("[bold green]Import Complete![/bold green]")

    except Exception as e:  # noqa: BLE001
        console.print(f"[bold red]Import Failed:[/bold red] {e}")


@ibkr_porez.command()
@click.option("--year", type=int, help="Filter by year (e.g. 2023)")
@click.option("-t", "--ticker", type=str, help="Show detailed breakdown for specific ticker")
@click.option(
    "-m",
    "--month",
    type=str,
    help="Show detailed breakdown for specific month (YYYY-MM, YYYYMM, or MM)",
)
def show(year: int | None, ticker: str | None, month: str | None):  # noqa: C901,PLR0912,PLR0915
    """Show tax report (Sales only)."""
    import re
    from collections import defaultdict
    from datetime import datetime
    from decimal import Decimal

    import pandas as pd
    from rich.console import Console
    from rich.table import Table

    console = Console()

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


@ibkr_porez.command()
@click.option("--half", required=False, help="Half-year to report (e.g. 2023-1, 20231)")
def report(half: str | None):  # noqa: C901,PLR0912,PLR0915
    """Generate PPDG-3R XML report."""
    import re
    from datetime import datetime

    from rich.console import Console

    console = Console()

    # Determine Period
    target_year = None
    target_half = None

    if half:
        # Parse Argument
        # Formats: 2023-2, 20232
        m_dash = re.match(r"^(\d{4})-(\d)$", half)
        m_compact = re.match(r"^(\d{4})(\d)$", half)

        if m_dash:
            target_year = int(m_dash.group(1))
            target_half = int(m_dash.group(2))
        elif m_compact:
            target_year = int(m_compact.group(1))
            target_half = int(m_compact.group(2))
        else:
            console.print(
                f"[red]Invalid format: {half}. Use YYYY-H (e.g. 2023-2) "
                f"or YYYYH (e.g. 20232)[/red]",
            )
            return

        if target_half not in [1, 2]:
            console.print("[red]Half-year must be 1 or 2.[/red]")
            return
    else:
        # Default: Last COMPLETE half-year
        now = datetime.now()
        current_year = now.year
        current_month = now.month

        if current_month < 7:  # noqa: PLR2004
            # Current is H1 (incomplete), so Last Complete is Previous Year H2
            target_year = current_year - 1
            target_half = 2
        else:
            # Current is H2 (incomplete), so Last Complete is Current Year H1
            target_year = current_year
            target_half = 1

    # Calculate Dates
    if target_half == 1:
        start_date = date(target_year, 1, 1)
        end_date = date(target_year, 6, 30)
    else:
        start_date = date(target_year, 7, 1)
        end_date = date(target_year, 12, 31)

    console.print(
        f"[bold blue]Generating Report for {target_year} H{target_half} "
        f"({start_date} to {end_date})[/bold blue]",
    )

    from ibkr_porez.report import XMLGenerator

    cfg = config_manager.load_config()
    storage = Storage()
    nbs = NBSClient(storage)
    tax_calc = TaxCalculator(nbs)
    xml_gen = XMLGenerator(cfg)

    # Get Transactions (DataFrame)
    # Load ALL to ensure FIFO context
    df_transactions = storage.get_transactions()

    if df_transactions.empty:
        console.print(
            "[yellow]No transactions found. Run `ibkr-porez get` first.[/yellow]",
        )
        return

    # Process FIFO for all
    all_entries = tax_calc.process_trades(df_transactions)

    # Filter for Period
    entries = []
    for e in all_entries:
        if start_date <= e.sale_date <= end_date:
            entries.append(e)

    if not entries:
        console.print("[yellow]No taxable sales found in this period.[/yellow]")
        return

    # Generate XML
    xml_content = xml_gen.generate_xml(entries, start_date, end_date)

    filename = f"ppdg3r_{target_year}_H{target_half}.xml"
    with open(filename, "w") as f:
        f.write(xml_content)

    console.print(f"[bold green]Report generated: {filename}[/bold green]")
    console.print(f"Total Entries: {len(entries)}")

    # Print Detailed Table for Manual Entry
    from rich.table import Table

    table = Table(title="Manual Entry Helpers (Part 4)", box=None)

    table.add_column("No.", justify="right")
    table.add_column("Ticker (Naziv)", justify="left")
    table.add_column("Sale Date (4.3)", justify="left")
    table.add_column("Qty (4.5/4.9)", justify="right")
    table.add_column("Sale Price RSD (4.6)", justify="right")  # Prodajna Cena
    table.add_column("Buy Date (4.7)", justify="left")
    table.add_column("Buy Price RSD (4.10)", justify="right")  # Nabavna Cena
    table.add_column("Gain RSD", justify="right")
    table.add_column("Loss RSD", justify="right")

    i = 1
    for e in entries:
        gain = e.capital_gain_rsd
        g_str = f"{gain:.2f}" if gain >= 0 else "0.00"
        l_str = f"{abs(gain):.2f}" if gain < 0 else "0.00"

        table.add_row(
            str(i),
            e.ticker,
            e.sale_date.strftime("%Y-%m-%d"),
            f"{e.quantity:.2f}",
            f"{e.sale_value_rsd:.2f}",
            e.purchase_date.strftime("%Y-%m-%d"),
            f"{e.purchase_value_rsd:.2f}",
            g_str,
            l_str,
        )
        i += 1

    console.print(table)
    console.print(
        "[dim]Use these values to cross-check with the portal or fill manually if needed.[/dim]",
    )

    console.print("\n[bold red]ATTENTION: Step 8 (Upload)[/bold red]")
    console.print("The XML includes a placeholder for Part 8 (Evidence).")
    console.print(
        "[bold]You MUST manually upload your IBKR Activity Report (PDF) "
        "in 'Deo 8' on the ePorezi portal.[/bold]",
    )


if __name__ == "__main__":  # pragma: no cover
    ibkr_porez()  # pylint: disable=no-value-for-parameter
