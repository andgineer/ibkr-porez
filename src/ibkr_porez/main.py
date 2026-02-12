"""ibkr-porez."""

import logging
import sys
from datetime import datetime
from decimal import Decimal, InvalidOperation
from pathlib import Path
from tempfile import NamedTemporaryFile

import rich_click as click
from rich.console import Console
from rich.logging import RichHandler

from ibkr_porez import __version__
from ibkr_porez.config import config_manager
from ibkr_porez.declaration_manager import DeclarationManager
from ibkr_porez.error_handling import get_user_friendly_error_message
from ibkr_porez.gui.launcher import launch_gui_process
from ibkr_porez.logging_config import ERROR_LOG_FILE, setup_logger
from ibkr_porez.models import (
    DeclarationStatus,
    IncomeDeclarationEntry,
    TaxReportEntry,
)
from ibkr_porez.operation_config import execute_config_command
from ibkr_porez.operation_get import GetOperation
from ibkr_porez.operation_import import ImportOperation, ImportType
from ibkr_porez.operation_list import ListDeclarations
from ibkr_porez.operation_report import display_income_declaration, execute_report_command
from ibkr_porez.operation_report_tables import render_declaration_table
from ibkr_porez.operation_show_declaration import show_declaration
from ibkr_porez.operation_stat import ShowStatistics
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


def _validate_lookback_option(ctx, param, value):  # noqa: ARG001
    if value is None or ctx.resilient_parsing:
        return value
    if value <= 0:
        raise click.BadParameter("must be a positive integer")
    return value


def _parse_non_negative_decimal(value: str, option_name: str) -> Decimal:
    try:
        parsed = Decimal(value)
    except (InvalidOperation, TypeError, ValueError) as error:
        raise click.BadParameter(f"{option_name} must be a decimal number") from error
    if parsed < Decimal("0"):
        raise click.BadParameter(f"{option_name} must be non-negative")
    return parsed.quantize(Decimal("0.01"))


@click.group(
    invoke_without_command=True,
    epilog=(
        "\nRun without a command to start the GUI.\n"
        "Documentation: https://andgineer.github.io/ibkr-porez/"
    ),
)
@click.version_option(version=__version__, prog_name="ibkr-porez")
@click.pass_context
def ibkr_porez(ctx: click.Context) -> None:
    """Automated PPDG-3R and PP OPO tax reports for Interactive Brokers.

    Starts GUI when invoked without a subcommand.
    """
    if ctx.invoked_subcommand is None and len(sys.argv) == 1:
        try:
            launch_gui_process(console=console, app_version=__version__)
        except Exception as e:  # noqa: BLE001
            raise click.ClickException(f"Failed to start GUI: {e}") from e


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#configuration-config",
)
@verbose_option
def config():
    """Configure IBKR and personal details."""
    execute_config_command(console)


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#fetch-data-fetch",
)
@verbose_option
def fetch():
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
            logger.exception("Error in fetch command")
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
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#view-declaration-details-show",
)
@click.argument("declaration_id", type=str)
@verbose_option
def show(declaration_id: str):
    """Show declaration details by ID."""
    show_declaration(declaration_id, console)


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#view-statistics-stat",
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
@click.option(
    "-o",
    "--output",
    "output_dir",
    type=str,
    required=False,
    help="Output directory (default: from config or Downloads)",
)
@click.option(
    "--lookback",
    "-l",
    "forced_lookback_days",
    type=int,
    callback=_validate_lookback_option,
    required=False,
    help=(
        "Look back N days to find missing declarations, ignoring saved last sync date. "
        "If omitted, sync continues from saved last sync date "
        f"(or {SyncOperation.DEFAULT_FIRST_SYNC_LOOKBACK_DAYS} days on first sync)."
    ),
)
@verbose_option
def sync(output_dir: str | None, forced_lookback_days: int | None):
    """Sync data from IBKR and create all necessary declarations."""
    cfg = config_manager.load_config()
    if not cfg.ibkr_token or not cfg.ibkr_query_id:
        console.print("[red]Missing Configuration! Run `ibkr-porez config` first.[/red]")
        return

    output_path = Path(output_dir) if output_dir else None
    operation = SyncOperation(
        cfg,
        output_dir=output_path,
        forced_lookback_days=forced_lookback_days,
    )
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

        console.print(
            "\n[dim]💡 You can now view declarations with `ibkr-porez list`, "
            "check details with `ibkr-porez show <id>`, "
            "and update status after submission/payment with `ibkr-porez submit <id>` "
            "/ `ibkr-porez pay <id>`[/dim]",
        )

    except Exception as e:  # noqa: BLE001
        # Log full traceback to error log
        logger.exception("Error in sync command")
        # Show user-friendly error message
        user_message = get_user_friendly_error_message(e)
        console.print(f"[bold red]Error:[/bold red] {user_message}")
        console.print(f"[dim]Full error details logged to: {ERROR_LOG_FILE}[/dim]")


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#generate-tax-report-report",
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
@click.option(
    "-o",
    "--output",
    "output_dir",
    type=str,
    required=False,
    help="Output directory (default: from config or Downloads)",
)
@verbose_option
def report(  # noqa: PLR0913
    type: str,
    half: str | None,
    start_date: str | None,
    end_date: str | None,
    force: bool,
    output_dir: str | None,
):
    """Generate tax reports (PPDG-3R for capital gains or PP OPO for capital income)."""
    output_path = Path(output_dir) if output_dir else None
    execute_report_command(
        type,
        half,
        start_date,
        end_date,
        console,
        force=force,
        output_dir=output_path,
    )


@ibkr_porez.command(
    "export-flex",
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#export-flex-query-export-flex",
)
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


@ibkr_porez.command(
    "list",
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#list-declarations-list",
)
@click.option("--all", is_flag=True, help="Show all declarations including submitted/paid")
@click.option(
    "--status",
    type=click.Choice(
        ["draft", "submitted", "pending", "finalized"],
        case_sensitive=False,
    ),
    help="Filter by status",
)
@click.option(
    "-1",
    "--ids-only",
    "ids_only",
    is_flag=True,
    help="Output only declaration IDs (one per line, for piping)",
)
@verbose_option
def list_declarations(all: bool, status: str | None, ids_only: bool):
    """List declarations (default: active = draft + submitted + pending)."""
    controller = ListDeclarations()
    status_enum = DeclarationStatus(status.lower()) if status else None
    result = controller.generate(show_all=all, status=status_enum, ids_only=ids_only)

    if ids_only:
        # result is list[str] when ids_only=True
        assert isinstance(result, list), "Expected list when ids_only=True"
        for decl_id in result:
            print(decl_id)
    else:
        # result is Table when ids_only=False
        console.print(result)


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#submit-declaration-submit",
)
@click.argument("declaration_id", type=str)
@verbose_option
def submit(declaration_id: str):
    """Mark declaration as submitted (imported to tax portal).

    Example: ibkr-porez submit 1
    Example: ibkr-porez list --status draft -1 | xargs -I {} ibkr-porez submit {}
    """
    manager = DeclarationManager()

    try:
        manager.submit([declaration_id])
        updated = manager.storage.get_declaration(declaration_id)
        if updated is not None and updated.status == DeclarationStatus.FINALIZED:
            console.print(f"[green]Finalized: {declaration_id} (no tax to pay)[/green]")
        elif updated is not None and updated.status == DeclarationStatus.PENDING:
            console.print(
                f"[green]Submitted: {declaration_id} (pending tax authority assessment)[/green]",
            )
        else:
            console.print(f"[green]Submitted: {declaration_id}[/green]")
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        raise click.ClickException(str(e)) from e


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#pay-declaration-pay",
)
@click.argument("declaration_id", type=str)
@click.option(
    "--tax",
    "tax_str",
    type=str,
    help="Record tax due amount (RSD) while marking declaration as paid",
)
@verbose_option
def pay(declaration_id: str, tax_str: str | None):
    """Mark declaration as paid.

    Example: ibkr-porez pay 1
    Example: ibkr-porez list --status draft -1 | xargs -I {} ibkr-porez pay {}
    """
    manager = DeclarationManager()

    try:
        if tax_str is None:
            manager.pay([declaration_id])
            console.print(f"[green]Paid: {declaration_id}[/green]")
        else:
            tax_due_rsd = _parse_non_negative_decimal(tax_str, "--tax")
            manager.set_assessed_tax(
                declaration_id=declaration_id,
                tax_due_rsd=tax_due_rsd,
                mark_paid=True,
            )
            console.print(f"[green]Paid: {declaration_id} ({tax_due_rsd} RSD recorded)[/green]")
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        raise click.ClickException(str(e)) from e


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/",
)
@click.argument("declaration_id", type=str)
@click.option(
    "-t",
    "--tax-due",
    "tax_due_str",
    type=str,
    required=True,
    help="Official tax due in RSD from tax authority assessment",
)
@click.option(
    "--paid",
    "mark_paid",
    is_flag=True,
    help="Mark declaration as already paid and finalize it",
)
@verbose_option
def assess(declaration_id: str, tax_due_str: str, mark_paid: bool):
    """Set official assessed tax amount for a declaration."""
    manager = DeclarationManager()
    tax_due_rsd = _parse_non_negative_decimal(tax_due_str, "--tax-due")

    try:
        updated = manager.set_assessed_tax(
            declaration_id=declaration_id,
            tax_due_rsd=tax_due_rsd,
            mark_paid=mark_paid,
        )
        if mark_paid:
            console.print(
                f"[green]Assessment saved and paid: {declaration_id} ({tax_due_rsd} RSD)[/green]",
            )
        elif updated.status == DeclarationStatus.FINALIZED:
            console.print(
                f"[green]Assessment saved: {declaration_id} (no tax to pay)[/green]",
            )
        else:
            console.print(
                f"[green]Assessment saved: {declaration_id} ({tax_due_rsd} RSD to pay)[/green]",
            )
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        raise click.ClickException(str(e)) from e


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#export-declaration-export",
)
@click.argument("declaration_id")
@click.option(
    "-o",
    "--output",
    type=str,
    help="Output directory (default: from config or Downloads)",
)
@verbose_option
def export(declaration_id: str, output: str | None):
    """Export declaration XML and all attached files."""
    manager = DeclarationManager()
    try:
        output_dir = Path(output) if output else None
        xml_path, attached_paths = manager.export(declaration_id, output_dir)

        console.print(f"[green]Exported XML: {xml_path}[/green]")
        if attached_paths:
            console.print(f"[green]Exported {len(attached_paths)} attached file(s):[/green]")
            for path in attached_paths:
                console.print(f"  [green]{path}[/green]")
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        raise click.ClickException(str(e)) from e


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#revert-declaration-status-revert",
)
@click.argument("declaration_id", type=str)
@click.option(
    "--to",
    type=click.Choice(["draft", "submitted"], case_sensitive=False),
    default="draft",
    help="Target status",
)
@verbose_option
def revert(declaration_id: str, to: str):
    """Revert declaration status (e.g., finalized -> draft).

    Example: ibkr-porez revert 1
    Example: ibkr-porez list --status finalized -1 | xargs -I {} ibkr-porez revert {} --to draft
    """
    manager = DeclarationManager()

    try:
        target_status = DeclarationStatus(to.lower())
        manager.revert([declaration_id], target_status)
        console.print(f"[green]Reverted {declaration_id} to {to}[/green]")
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        raise click.ClickException(str(e)) from e


@ibkr_porez.command(
    epilog="\nDocumentation: https://andgineer.github.io/ibkr-porez/usage/#attach-file-to-declaration-attach",
)
@click.argument("declaration_id")
@click.argument("file_path", type=str, required=False)
@click.option("-d", "--delete", is_flag=True, help="Delete attached file instead of adding")
@click.option("--file-id", type=str, help="File identifier for deletion (default: filename)")
@verbose_option
def attach(declaration_id: str, file_path: str | None, delete: bool, file_id: str | None):
    """
    Attach or remove file from declaration.

    To attach: ibkr-porez attach <declaration_id> <file_path>
    To remove: ibkr-porez attach <declaration_id> <file_id> --delete
    """
    manager = DeclarationManager()

    try:
        if delete:
            # Remove file
            file_identifier = file_id or file_path
            if not file_identifier:
                console.print("[red]File identifier required for deletion[/red]")
                raise click.ClickException("File identifier required for deletion")
            manager.detach_file(declaration_id, file_identifier)
            console.print(
                f"[green]Removed file '{file_identifier}' "
                f"from declaration {declaration_id}[/green]",
            )
        else:
            # Add file
            if not file_path:
                console.print("[red]File path required[/red]")
                raise click.ClickException("File path required")
            source_path = Path(file_path)
            if not source_path.exists():
                console.print(f"[red]File not found: {file_path}[/red]")
                raise click.ClickException(f"File not found: {file_path}")

            file_identifier = manager.attach_file(declaration_id, source_path)
            console.print(
                f"[green]Attached file '{file_identifier}' to declaration {declaration_id}[/green]",
            )
    except ValueError as e:
        console.print(f"[red]{e}[/red]")
        raise click.ClickException(str(e)) from e


if __name__ == "__main__":  # pragma: no cover
    ibkr_porez()  # pylint: disable=no-value-for-parameter
