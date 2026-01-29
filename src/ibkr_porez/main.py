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
def show():
    """Show monthly statistics."""
    from collections import defaultdict

    from rich.console import Console
    from rich.table import Table

    console = Console()

    storage = Storage()
    nbs = NBSClient(storage)
    tax_calc = TaxCalculator(nbs)

    df_transactions = storage.get_transactions()
    if df_transactions.empty:
        console.print("[yellow]No transactions found. Run `ibkr-porez get`.[/yellow]")
        return

    # Group by Month-Year and Ticker
    # Structure: { "YYYY-MM": { "TICKER": { "divs": 0, "sales_count": 0, "pnl": 0 } } }
    stats = defaultdict(lambda: defaultdict(lambda: {"divs": 0, "sales_count": 0, "pnl": 0}))

    # Process Taxable Sales
    sales_entries = tax_calc.process_trades(df_transactions)

    for entry in sales_entries:
        month_key = entry.sale_date.strftime("%Y-%m")
        ticker = entry.ticker
        stats[month_key][ticker]["sales_count"] += 1
        stats[month_key][ticker]["pnl"] += entry.capital_gain_rsd

    # Process Dividends
    if "type" in df_transactions.columns:
        divs = df_transactions[df_transactions["type"] == "DIVIDEND"].copy()

        # Pre-fetch rates for dividends logic?
        # Re-use existing simple logic but fix iteration
        for _, row in divs.iterrows():
            d = row["date"]  # date object
            curr = row["currency"]
            amt = float(row["amount"])
            ticker = row["symbol"]

            # Rate
            from ibkr_porez.models import Currency

            try:
                c_enum = Currency(curr)
                rate = nbs.get_rate(d, c_enum)
                if rate:
                    val = amt * float(rate)
                    month_key = d.strftime("%Y-%m")
                    stats[month_key][ticker]["divs"] += val
            except ValueError:
                pass

    # Print Table
    table = Table(title="Monthly Report Breakdown")
    table.add_column("Month", justify="left")
    table.add_column("Ticker", justify="left")
    table.add_column("Dividends (RSD)", justify="right")
    table.add_column("Sales Count", justify="right")
    table.add_column("Realized P/L (RSD)", justify="right")

    # Flatten and Sort
    # List of (Month, Ticker, Stats)
    rows = []
    for month, tickers in stats.items():
        for ticker, data in tickers.items():
            rows.append((month, ticker, data))

    # Sort by Month DESC, Ticker ASC
    rows.sort(
        key=lambda x: (x[0], x[1]), reverse=True
    )  # Sort logic slightly tricky with mixed direction?
    # Actually User requested "Month -> different papers inside".
    # Sort by Month DESC is good. Ticker ASC inside.

    rows.sort(key=lambda x: x[0], reverse=True)  # Sort by Month Desc
    # Then sort stable? No, python sort is stable.
    # To get Month DESC + Ticker ASC:
    # Sort by Ticker ASC first, then Month DESC.
    rows.sort(key=lambda x: x[1])  # Ticker ASC
    rows.sort(key=lambda x: x[0], reverse=True)  # Month DESC

    current_month = None
    for month, ticker, data in rows:
        if current_month != month:
            table.add_section()
            current_month = month

        table.add_row(
            month,
            ticker,
            f"{data['divs']:,.2f}",
            str(data["sales_count"]),
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
