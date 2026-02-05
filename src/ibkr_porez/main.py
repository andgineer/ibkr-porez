"""ibkr-porez."""

import logging
import sys
from datetime import datetime
from pathlib import Path
from tempfile import NamedTemporaryFile

import rich_click as click
from platformdirs import user_data_dir
from rich.console import Console
from rich.logging import RichHandler

from ibkr_porez import __version__
from ibkr_porez.config import UserConfig, config_manager
from ibkr_porez.error_handling import get_user_friendly_error_message
from ibkr_porez.logging_config import ERROR_LOG_FILE, setup_logger
from ibkr_porez.models import IncomeDeclarationEntry, TaxReportEntry
from ibkr_porez.operation_get import GetOperation
from ibkr_porez.operation_import import ImportOperation, ImportType
from ibkr_porez.operation_report import display_income_declaration, execute_report_command
from ibkr_porez.operation_report_tables import render_declaration_table
from ibkr_porez.operation_show import ShowStatistics
from ibkr_porez.operation_show_declaration import show_declaration
from ibkr_porez.operation_sync import SyncOperation
from ibkr_porez.storage import Storage
from ibkr_porez.storage_flex_queries import restore_report

OUTPUT_FILE_DEFAULT = "output"


def _get_output_folder() -> Path:
    """Get output folder from config or default to Downloads."""
    config = config_manager.load_config()
    if config.output_folder:
        return Path(config.output_folder)
    return Path.home() / "Downloads"


# Global Console instance to ensure logs and progress bars share the same stream
console = Console()

# Setup file logger for error logging
logger = setup_logger()


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

    default_data_dir = current_config.data_dir or str(
        Path(user_data_dir("ibkr-porez")) / Storage.DATA_SUBDIR,
    )
    data_dir_input = click.prompt(
        "Data Directory (absolute path to folder with transactions.json, "
        "default: ibkr-porez-data in app folder)",
        default=default_data_dir,
        show_default=True,
    )
    # If user entered the default ibkr-porez-data folder, set to None to use default
    default_ibkr_porez_data = str(Path(user_data_dir("ibkr-porez")) / Storage.DATA_SUBDIR)
    data_dir = (
        None
        if data_dir_input.strip() == default_ibkr_porez_data
        else (data_dir_input.strip() if data_dir_input.strip() else None)
    )

    default_output_folder = current_config.output_folder or str(Path.home() / "Downloads")
    output_folder_input = click.prompt(
        "Output Folder (absolute path to folder for saving files from sync, export, "
        "export-flex, report commands, default: Downloads)",
        default=default_output_folder,
        show_default=True,
    )
    output_folder = (
        None
        if output_folder_input.strip() == str(Path.home() / "Downloads")
        else (output_folder_input.strip() if output_folder_input.strip() else None)
    )

    new_config = UserConfig(
        ibkr_token=ibkr_token,
        ibkr_query_id=ibkr_query_id,
        personal_id=personal_id,
        full_name=full_name,
        address=address,
        city_code=city_code,
        phone=phone,
        email=email,
        data_dir=data_dir,
        output_folder=output_folder,
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

    operation = GetOperation(cfg)

    with console.status("[bold green]Fetching data from IBKR...[/bold green]"):
        try:
            console.print("[blue]Fetching full report...[/blue]")
            transactions, count_inserted, count_updated = operation.execute()
            msg = f"Fetched {len(transactions)} transactions."
            stats = f"({count_inserted} new, {count_updated} updated)"
            console.print(f"[green]{msg} {stats}[/green]")

        except Exception as e:  # noqa: BLE001
            # Log full traceback to error log
            logger.exception("Error in get command")
            # Show user-friendly error message
            user_message = get_user_friendly_error_message(e)
            console.print(f"[bold red]Error:[/bold red] {user_message}")
            console.print(f"[dim]Full error details logged to: {ERROR_LOG_FILE}[/dim]")
            return

    # Rate sync is already done in process_flex_query
    console.print("[bold green]Sync Complete![/bold green]")


def _determine_input_source(file_path: str | None) -> tuple[bool, Path]:
    """
    Determine input source (stdin or file) and return path.

    Returns:
        tuple[bool, Path]: (read_from_stdin, file_path)
    """
    # Check if we should read from stdin
    if file_path is None or file_path == "-":
        return True, Path()

    file_path_obj = Path(file_path)
    # If file doesn't exist and stdin is piped, use stdin
    if not file_path_obj.exists() and not sys.stdin.isatty():
        return True, Path()

    if not file_path_obj.exists():
        raise FileNotFoundError(f"File not found: {file_path}")

    return False, file_path_obj


@ibkr_porez.command(
    "import",
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#import-historical-data-import",
)
@click.argument("file_path", type=str, required=False)
@click.option(
    "-t",
    "--type",
    "import_type",
    type=click.Choice(["auto", "csv", "flex"], case_sensitive=False),
    default="auto",
    help="Import type: 'auto' (detect from file), 'csv', or 'flex' (XML)",
)
@verbose_option
def import_file(file_path: str | None, import_type: str):
    """Import historical transactions from CSV Activity Statement or Flex Query XML."""
    operation = ImportOperation()

    # Convert string to ImportType enum
    type_map = {
        "auto": ImportType.AUTO,
        "csv": ImportType.CSV,
        "flex": ImportType.FLEX,
    }
    import_type_enum = type_map.get(import_type.lower(), ImportType.AUTO)

    read_from_stdin = False
    input_path = Path()
    try:
        # Determine input source
        read_from_stdin, input_path = _determine_input_source(file_path)

        if read_from_stdin:
            console.print("[blue]Importing from stdin...[/blue]")
            # Read from stdin and create temporary file
            content = sys.stdin.read()

            with NamedTemporaryFile(mode="w", suffix=".xml", delete=False) as tmp_file:
                tmp_file.write(content)
                input_path = Path(tmp_file.name)
        else:
            console.print(f"[blue]Importing from {file_path}...[/blue]")

        with console.status("[bold green]Processing import...[/bold green]"):
            transactions, count_inserted, count_updated = operation.execute(
                input_path,
                import_type_enum,
            )

        # Clean up temp file if we created one
        if read_from_stdin and input_path.exists():
            input_path.unlink()

        if not transactions:
            console.print("[yellow]No valid transactions found in file.[/yellow]")
            return

        msg = f"Parsed {len(transactions)} transactions."
        stats = f"({count_inserted} new, {count_updated} updated)"
        console.print(f"[green]{msg} {stats}[/green]")

        # Rate sync is already done in ImportOperation
        console.print("[bold green]Import Complete![/bold green]")

    except FileNotFoundError as e:
        console.print(f"[bold red]{e}[/bold red]")
    except Exception as e:  # noqa: BLE001
        # Clean up temp file on error
        if read_from_stdin and input_path.exists():
            input_path.unlink()
        # Log full traceback to error log
        logger.exception("Error in import command")
        # Show user-friendly error message
        user_message = get_user_friendly_error_message(e)
        console.print(f"[bold red]Import Failed:[/bold red] {user_message}")
        console.print(f"[dim]Full error details logged to: {ERROR_LOG_FILE}[/dim]")


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#show-declaration-show",
)
@click.argument("declaration_id", type=str)
@verbose_option
def show(declaration_id: str):
    """Show declaration details by ID."""
    show_declaration(declaration_id, console)


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#show-statistics-stat",
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
def stat(year: int | None, ticker: str | None, month: str | None):
    """Show transaction statistics with optional filters."""
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
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#sync-data-and-create-declarations-sync",
)
@verbose_option
def sync():
    """Sync data from IBKR and create all necessary declarations."""
    cfg = config_manager.load_config()
    if not cfg.ibkr_token or not cfg.ibkr_query_id:
        console.print("[red]Missing Configuration! Run `ibkr-porez config` first.[/red]")
        return

    operation = SyncOperation(cfg)
    try:
        with console.status("[bold green]Syncing data and creating declarations...[/bold green]"):
            declarations = operation.execute()

        if not declarations:
            console.print("[yellow]No new declarations created.[/yellow]")
            return

        console.print(f"\n[bold green]Created {len(declarations)} declaration(s):[/bold green]")
        for decl in declarations:
            console.print(f"  [green]{decl.declaration_id}[/green]")
            if decl.report_data:
                if decl.type.value == "PPDG-3R":
                    # Gains: render table
                    gains_entries = [e for e in decl.report_data if isinstance(e, TaxReportEntry)]
                    if gains_entries:
                        table = render_declaration_table(gains_entries)
                        console.print(table)
                else:
                    # Income: show declaration fields
                    for entry in decl.report_data:
                        if isinstance(entry, IncomeDeclarationEntry):
                            display_income_declaration(entry, console)

    except Exception as e:  # noqa: BLE001
        # Log full traceback to error log
        logger.exception("Error in sync command")
        # Show user-friendly error message
        user_message = get_user_friendly_error_message(e)
        console.print(f"[bold red]Error:[/bold red] {user_message}")
        console.print(f"[dim]Full error details logged to: {ERROR_LOG_FILE}[/dim]")


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
@click.option(
    "--force",
    is_flag=True,
    default=False,
    help=(
        "For income reports: create declaration with zero withholding tax "
        "even if tax not found. Warning: this means you'll need to pay tax in Serbia."
    ),
)
@verbose_option
def report(
    type: str,
    half: str | None,
    start_date: str | None,
    end_date: str | None,
    force: bool,
):
    """Generate tax reports (PPDG-3R for capital gains or PP OPO for capital income)."""
    execute_report_command(type, half, start_date, end_date, console, force=force)


@ibkr_porez.command("export-flex")
@click.argument("date", type=str)
@click.option(
    "-o",
    "--output",
    "output_path",
    type=str,
    required=False,
    help=(
        "Output file path or '-' for stdout (default: flex_query_YYYYMMDD.xml in current directory)"
    ),
)
@verbose_option
def export_flex(date: str, output_path: str | None):
    """Export full flex query XML file for the given date."""
    # Parse date
    try:
        report_date = datetime.strptime(date, "%Y-%m-%d").date()
    except ValueError as e:
        console.print(f"[bold red]Invalid date format:[/bold red] {date}")
        console.print("[dim]Use YYYY-MM-DD format (e.g. 2026-01-29)[/dim]")
        raise click.BadParameter(f"Invalid date format: {date}. Use YYYY-MM-DD") from e

    storage = Storage()

    try:
        # Export the report
        restored_content = restore_report(storage, report_date)

        if restored_content is None:
            console.print(
                f"[bold red]No flex query found for date {date}[/bold red]",
            )
            return

        # Determine output: stdout if -o - or if output_path is None and stdout is not a TTY (piped)
        write_to_stdout = False
        if output_path == "-":
            write_to_stdout = True
        elif output_path is None:
            # Auto-detect: if stdout is piped (not a TTY), write to stdout
            write_to_stdout = not sys.stdout.isatty()

        if write_to_stdout:
            # Write to stdout (for piping)
            sys.stdout.write(restored_content)
            sys.stdout.flush()
        else:
            # Write to file
            if output_path is None:
                output_folder = _get_output_folder()
                output_folder.mkdir(parents=True, exist_ok=True)
                file_path = output_folder / f"flex_query_{report_date.strftime('%Y%m%d')}.xml"
            else:
                file_path = Path(output_path)
                # Ensure .xml extension if not provided
                if file_path.suffix != ".xml":
                    file_path = file_path.with_suffix(".xml")
            file_path.write_text(restored_content, encoding="utf-8")
            console.print(
                f"[bold green]Exported flex query saved to:[/bold green] {file_path.absolute()}",
            )

    except Exception as e:  # noqa: BLE001
        # Log full traceback to error log
        logger.exception("Error in export-flex command")
        # Show user-friendly error message
        user_message = get_user_friendly_error_message(e)
        console.print(f"[bold red]Error:[/bold red] {user_message}")
        console.print(f"[dim]Full error details logged to: {ERROR_LOG_FILE}[/dim]")


if __name__ == "__main__":  # pragma: no cover
    ibkr_porez()  # pylint: disable=no-value-for-parameter
