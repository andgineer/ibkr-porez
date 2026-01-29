"""ibkr-porez."""

from datetime import date

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

    console = click.get_current_context().find_root().info_name
    from rich.console import Console

    console = Console()

    console.print("[bold blue]Configuration Setup[/bold blue]")
    console.print(f"Config file location: {config_manager.config_path}\n")

    ibkr_token = click.prompt("IBKR Flex Token", default=current_config.ibkr_token)
    ibkr_query_id = click.prompt("IBKR Query ID", default=current_config.ibkr_query_id)

    personal_id = click.prompt("Personal Search ID (JMBG)", default=current_config.personal_id)
    full_name = click.prompt("Full Name", default=current_config.full_name)
    address = click.prompt("Address", default=current_config.address)

    new_config = UserConfig(
        ibkr_token=ibkr_token,
        ibkr_query_id=ibkr_query_id,
        personal_id=personal_id,
        full_name=full_name,
        address=address,
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
                    f"[blue]Found existing data up to {last_date}. Fetching updates...[/blue]"
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
            storage.save_transactions(transactions)
            console.print(f"[green]Saved {len(transactions)} transactions.[/green]")

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

        except Exception as e:
            console.print(f"[bold red]Error:[/bold red] {e}")
            import traceback

            traceback.print_exc()


@ibkr_porez.command()
@click.option("--year", type=int, help="Filter by year (e.g. 2023)")
@click.option("-t", "--ticker", type=str, help="Show detailed breakdown for specific ticker")
@click.option("-m", "--month", type=str, help="Show detailed breakdown for specific month (YYYY-MM, YYYYMM, or MM)")
def show(year: int | None, ticker: str | None, month: str | None):
    """Show tax report (Sales only)."""
    from collections import defaultdict
    from rich.console import Console
    from rich.table import Table
    from decimal import Decimal
    import re
    import pandas as pd
    from datetime import datetime

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
                # Ideally yes, but let's stick to sales for the detailed view context or generally present data.
                # Let's check dividends too.
                if "type" in df_transactions.columns:
                     divs_check = df_transactions[df_transactions["type"] == "DIVIDEND"]
                     for d in pd.to_datetime(divs_check["date"]).dt.date:
                         if d.month == target_month:
                             years_with_data.add(d.year)

                if years_with_data:
                    target_year = max(years_with_data)
                else:
                    # Default to current year or error?
                    # Let's pick current year if no data found, so user sees empty.
                    target_year = datetime.now().year
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
            if ticker: msg += f" ticker={ticker}"
            if target_year: msg += f" year={target_year}"
            if target_month: msg += f" month={target_month}"
            msg += "[/yellow]"
            console.print(msg)
            return

        title_parts = []
        if ticker: title_parts.append(ticker)
        if target_year:
            if target_month:
                title_parts.append(f"{target_year}-{target_month:02d}")
            else:
                title_parts.append(str(target_year))
        
        table_title = f"Detailed Report: {' - '.join(title_parts)}"
        table = Table(title=table_title, box=None) # Cleaner look
        
        table.add_column("Sale Date", justify="left")
        table.add_column("Qty", justify="right")
        table.add_column("Sale Price", justify="right")
        table.add_column("Sale Rate", justify="right")
        table.add_column("Sale Val (RSD)", justify="right") # ADDED
        
        table.add_column("Buy Date", justify="left")
        table.add_column("Buy Price", justify="right")
        table.add_column("Buy Rate", justify="right")
        table.add_column("Buy Val (RSD)", justify="right") # ADDED
        
        table.add_column("Gain (RSD)", justify="right")

        total_pnl = Decimal(0)

        for e in filtered_entries:
            total_pnl += e.capital_gain_rsd
            table.add_row(
                str(e.sale_date),
                f"{e.quantity:.2f}",
                f"{e.sale_price:.2f}",
                f"{e.sale_exchange_rate:.4f}",
                f"{e.sale_value_rsd:,.0f}", # No decimals for large RSD values usually cleaner, or .2f
                str(e.purchase_date),
                f"{e.purchase_price:.2f}",
                f"{e.purchase_exchange_rate:.4f}",
                f"{e.purchase_value_rsd:,.0f}",
                f"[bold]{e.capital_gain_rsd:,.2f}[/bold]"
            )
            
        console.print(table)
        console.print(f"[bold]Total P/L: {total_pnl:,.2f} RSD[/bold]")
        return

    # Fallback to Aggregated View (Summary)
    # Group by Month-Year and Ticker
    # Structure: { "YYYY-MM": { "TICKER": { "divs": 0, "sales_count": 0, "pnl": 0 } } }
    stats = defaultdict(lambda: defaultdict(lambda: {"divs": 0, "sales_count": 0, "pnl": 0}))

    for entry in sales_entries: # Already filtered by year (if --year passed, but maybe not by -m)
        if target_year and entry.sale_date.year != target_year:
            continue
        if target_month and entry.sale_date.month != target_month:
            continue
        if ticker and entry.ticker != ticker: # Should be handled by Detail view usually, but keeping logic safely
            continue
            
        month_key = entry.sale_date.strftime("%Y-%m")
        t = entry.ticker
        stats[month_key][t]["sales_count"] += 1
        stats[month_key][t]["pnl"] += entry.capital_gain_rsd

    # Process Dividends
    if "type" in df_transactions.columns:
        divs = df_transactions[df_transactions["type"] == "DIVIDEND"].copy()
        
        for _, row in divs.iterrows():
            d = row["date"] # date object
            if target_year and d.year != target_year:
                continue
            if target_month and d.month != target_month:
                 continue
            
            t = row["symbol"]
            if ticker and t != ticker:
                continue
                
            curr = row["currency"]
            amt = float(row["amount"])
            
            # Rate
            from ibkr_porez.models import Currency
            try:
                c_enum = Currency(curr)
                rate = nbs.get_rate(d, c_enum)
                if rate:
                    val = amt * float(rate)
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
            
    rows.sort(key=lambda x: x[1]) # Ticker ASC
    rows.sort(key=lambda x: x[0], reverse=True) # Month DESC

    current_month = None
    for m, t, data in rows:
        if current_month != m:
            table.add_section()
            current_month = m
            
        table.add_row(
            m,
            t,
            f"{data['divs']:,.2f}",
            str(data['sales_count']),
            f"{data['pnl']:,.2f}",
        )

    console.print(table)


@ibkr_porez.command()
@click.option("--year", type=int, required=True, help="Year to report (e.g. 2023)")
@click.option("--half", type=click.Choice(["1", "2"]), required=True, help="Half-year (1 or 2)")
def report(year: int, half: str):
    """Generate PPDG-3R XML report."""
    from rich.console import Console

    console = Console()

    from ibkr_porez.report import XMLGenerator

    # Calculate Period
    if half == "1":
        start_date = date(year, 1, 1)
        end_date = date(year, 6, 30)
    else:
        start_date = date(year, 7, 1)
        end_date = date(year, 12, 31)

    console.print(f"[bold blue]Generating Report for {start_date} to {end_date}[/bold blue]")

    cfg = config_manager.load_config()
    storage = Storage()
    nbs = NBSClient(storage)
    tax_calc = TaxCalculator(nbs)
    xml_gen = XMLGenerator(cfg)

    # Get Transactions (DataFrame)
    df_transactions = storage.get_transactions(start_date, end_date)

    if df_transactions.empty:
        console.print(
            "[yellow]No transactions found for this period. Run `ibkr-porez get` first.[/yellow]",
        )
        return

    # Process
    entries = tax_calc.process_trades(df_transactions)
    if not entries:
        console.print("[yellow]No taxable sales found in this period.[/yellow]")
        return

    # Generate XML
    xml_content = xml_gen.generate_xml(entries, start_date, end_date)

    filename = f"ppdg3r_{year}_H{half}.xml"
    with open(filename, "w") as f:
        f.write(xml_content)

    console.print(f"[bold green]Report generated: {filename}[/bold green]")
    console.print(f"Total Entries: {len(entries)}")


if __name__ == "__main__":  # pragma: no cover
    ibkr_porez()  # pylint: disable=no-value-for-parameter
