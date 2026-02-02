"""ibkr-porez."""

import logging
import re
import time
from pathlib import Path

import rich_click as click
from pydantic import ValidationError
from rich.console import Console
from rich.logging import RichHandler
from rich.progress import track

from ibkr_porez import __version__
from ibkr_porez.config import UserConfig, config_manager
from ibkr_porez.ibkr_csv import CSVParser
from ibkr_porez.ibkr_flex_query import IBKRClient
from ibkr_porez.nbs import NBSClient
from ibkr_porez.report_gains import GainsReportGenerator
from ibkr_porez.report_income import IncomeReportGenerator
from ibkr_porez.report_params import ReportParams, ReportType
from ibkr_porez.show_statistics import ShowStatistics
from ibkr_porez.storage import Storage
from ibkr_porez.tables import render_declaration_table
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
@click.option("-y", "--year", type=int, help="Filter by year (e.g. 2026)")
@click.option("-t", "--ticker", type=str, help="Show detailed breakdown for specific ticker")
@click.option(
    "-m",
    "--month",
    type=str,
    help="Show detailed breakdown for specific month (YYYY-MM, YYYYMM, or MM)",
)
@verbose_option
def show(year: int | None, ticker: str | None, month: str | None):
    """Show tax report (Sales only)."""
    generator = ShowStatistics()

    try:
        table, total_pnl, error_msg = generator.generate(year=year, ticker=ticker, month=month)
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        return

    if error_msg:
        console.print(f"[yellow]{error_msg}[/yellow]")
        return

    if table is None:
        return

    console.print(table)

    if total_pnl is not None:
        console.print(f"[bold]Total P/L: {total_pnl:,.2f} RSD[/bold]")


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#generate-capital-gains-tax-report-report",
)
@click.option(
    "-t",
    "--type",
    type=click.Choice(["gains", "income"], case_sensitive=False),
    default="gains",
    help="Report type: 'gains' for PPDG-3R (capital gains) or 'income' for PP OPO (capital income)",
)
@click.option(
    "-h",
    "--half",
    required=False,
    help=(
        "Half-year period (e.g. 2026-1, 20261). "
        "For --type=gains, defaults to the last complete half-year if not provided."
    ),
)
@click.option(
    "-s",
    "--start",
    "start_date",
    required=False,
    help=(
        "Start date (YYYY-MM-DD). "
        "If --start and --end are not provided, they default to current month "
        "(from 1st to today). If only --start is provided, --end defaults to --start."
    ),
)
@click.option(
    "-e",
    "--end",
    "end_date",
    required=False,
    help=(
        "End date (YYYY-MM-DD). "
        "If --start and --end are not provided, they default to current month "
        "(from 1st to today). If only --start is provided, --end defaults to --start."
    ),
)
@verbose_option
def report(  # noqa: C901,PLR0915, PLR0912
    type: str,
    half: str | None,
    start_date: str | None,
    end_date: str | None,
):  # noqa: PLR0913
    """Generate tax reports (PPDG-3R for capital gains or PP OPO for capital income)."""
    try:
        params = ReportParams.model_validate(
            {
                "type": type,
                "half": half,
                "start": start_date,
                "end": end_date,
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

            results = generator.generate(
                start_date=start_date_obj,
                end_date=end_date_obj,
                filename=filename,
            )

            if not results:
                console.print("[yellow]No declarations generated.[/yellow]")
                return

            console.print(f"[bold green]Generated {len(results)} declaration(s):[/bold green]")

            total_entries = 0
            for filename, entries in results:
                console.print(f"  [green]{filename}[/green] ({len(entries)} entries)")
                total_entries += len(entries)

                table = render_declaration_table(entries)
                console.print(table)
                console.print(
                    "[dim]Use these values to cross-check with the portal "
                    "or fill manually if needed.[/dim]",
                )

            console.print(f"\n[bold]Total Entries: {total_entries}[/bold]")

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
        console.print(
            f"[bold blue]Generating PP OPO Report for "
            f"({start_date_obj} to {end_date_obj})[/bold blue]",
        )

        try:
            generator = IncomeReportGenerator()
            results = generator.generate(
                start_date=start_date_obj,
                end_date=end_date_obj,
            )

            if not results:
                console.print("[yellow]No income declarations generated.[/yellow]")
                return

            console.print(f"[bold green]Generated {len(results)} declaration(s):[/bold green]")

            for filename, entries in results:
                console.print(f"  [green]{filename}[/green]")
                # Each entry represents one declaration (aggregated if multiple income sources)
                for entry in entries:
                    console.print(f"    Date: {entry.date.strftime('%Y-%m-%d')}")
                    console.print(f"    SifraVrstePrihoda: {entry.sifra_vrste_prihoda}")
                    console.print(f"    BrutoPrihod: {entry.bruto_prihod:,.2f} RSD")
                    console.print(f"    OsnovicaZaPorez: {entry.osnovica_za_porez:,.2f} RSD")
                    console.print(f"    ObracunatiPorez: {entry.obracunati_porez:,.2f} RSD")
                    console.print(
                        f"    PorezPlacenDrugojDrzavi: {entry.porez_placen_drugoj_drzavi:,.2f} RSD",
                    )
                    console.print(f"    PorezZaUplatu: {entry.porez_za_uplatu:,.2f} RSD")
                console.print(
                    "[dim]Use these values to cross-check with the portal "
                    "or fill manually if needed.[/dim]",
                )

        except ValueError as e:
            console.print(f"[yellow]{e}[/yellow]")
            return


if __name__ == "__main__":  # pragma: no cover
    ibkr_porez()  # pylint: disable=no-value-for-parameter
