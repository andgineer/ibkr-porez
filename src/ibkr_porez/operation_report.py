"""Report command implementation."""

import re
from datetime import date

from pydantic import ValidationError
from rich.console import Console

from ibkr_porez.models import IncomeDeclarationEntry
from ibkr_porez.operation_report_params import ReportParams, ReportType
from ibkr_porez.operation_report_tables import render_declaration_table
from ibkr_porez.report_gains import GainsReportGenerator
from ibkr_porez.report_income import IncomeReportGenerator
from ibkr_porez.validation import handle_validation_error


def generate_gains_filename(half: str | None) -> str | None:
    """
    Generate filename for gains report from half-year parameter.

    Args:
        half: Half-year period (e.g. 2026-1, 20261).

    Returns:
        Filename if half is provided and valid, None otherwise.
    """
    if not half:
        return None

    half_match = re.match(r"^(\d{4})-(\d)$", half) or re.match(r"^(\d{4})(\d)$", half)
    if half_match:
        target_year = int(half_match.group(1))
        target_half = int(half_match.group(2))
        return f"ppdg3r_{target_year}_H{target_half}.xml"

    return None


def process_gains_report(
    start_date: date,
    end_date: date,
    filename: str | None,
    console: Console,
) -> None:
    """
    Process and display gains report (PPDG-3R).

    Args:
        start_date: Start date for the report period.
        end_date: End date for the report period.
        filename: Optional filename for the report.
        console: Rich console for output.
    """
    console.print(
        f"[bold blue]Generating PPDG-3R Report for ({start_date} to {end_date})[/bold blue]",
    )

    try:
        generator = GainsReportGenerator()
        results = generator.generate(
            start_date=start_date,
            end_date=end_date,
            filename=filename,
        )

        declaration_count = 0
        total_entries = 0
        for filename_result, entries in results:
            if declaration_count == 0:
                console.print("[bold green]Generated declaration(s):[/bold green]")
            declaration_count += 1
            console.print(f"  [green]{filename_result}[/green] ({len(entries)} entries)")
            total_entries += len(entries)

            table = render_declaration_table(entries)
            console.print(table)

        if declaration_count == 0:
            console.print("[yellow]No declarations generated.[/yellow]")
            return

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


def display_income_declaration(entry: IncomeDeclarationEntry, console: Console) -> None:
    """
    Display a single income declaration entry.

    Args:
        entry: Income declaration entry to display.
        console: Rich console for output.
    """
    console.print(f"    Date: {entry.date.strftime('%Y-%m-%d')}")
    console.print(f"    SifraVrstePrihoda: {entry.sifra_vrste_prihoda}")
    console.print(f"    BrutoPrihod: {entry.bruto_prihod:,.2f} RSD")
    console.print(f"    OsnovicaZaPorez: {entry.osnovica_za_porez:,.2f} RSD")
    console.print(f"    ObracunatiPorez: {entry.obracunati_porez:,.2f} RSD")
    console.print(
        f"    PorezPlacenDrugojDrzavi: {entry.porez_placen_drugoj_drzavi:,.2f} RSD",
    )
    console.print(f"    PorezZaUplatu: {entry.porez_za_uplatu:,.2f} RSD")


def process_income_report(
    start_date: date,
    end_date: date,
    console: Console,
) -> None:
    """
    Process and display income report (PP OPO).

    Args:
        start_date: Start date for the report period.
        end_date: End date for the report period.
        console: Rich console for output.
    """
    console.print(
        f"[bold blue]Generating PP OPO Report for ({start_date} to {end_date})[/bold blue]",
    )

    try:
        generator = IncomeReportGenerator()
        results = generator.generate(
            start_date=start_date,
            end_date=end_date,
        )

        declaration_count = 0
        for filename, entries in results:
            if declaration_count == 0:
                console.print("[bold green]Generated declaration(s):[/bold green]")
            declaration_count += 1
            console.print(f"  [green]{filename}[/green]")
            # Each entry represents one declaration (aggregated if multiple income sources)
            for entry in entries:
                display_income_declaration(entry, console)

        if declaration_count == 0:
            console.print("[yellow]No income declarations generated.[/yellow]")
            return

    except ValueError as e:
        console.print(f"[yellow]{e}[/yellow]")
        return


def execute_report_command(
    type: str,
    half: str | None,
    start_date: str | None,
    end_date: str | None,
    console: Console,
) -> None:
    """
    Execute the report command.

    Args:
        type: Report type ('gains' or 'income').
        half: Half-year period (e.g. 2026-1, 20261).
        start_date: Start date string (YYYY-MM-DD).
        end_date: End date string (YYYY-MM-DD).
        console: Rich console for output.
    """
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
        filename = generate_gains_filename(params.half)
        process_gains_report(start_date_obj, end_date_obj, filename, console)
    elif params.type == ReportType.INCOME:
        process_income_report(start_date_obj, end_date_obj, console)
