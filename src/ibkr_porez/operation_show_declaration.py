"""Display declaration details."""

from io import StringIO

from rich.console import Console
from rich.table import Table

from ibkr_porez.models import DeclarationType, IncomeDeclarationEntry, TaxReportEntry
from ibkr_porez.operation_report import display_income_declaration
from ibkr_porez.operation_report_tables import render_declaration_table
from ibkr_porez.storage import Storage


def show_declaration(declaration_id: str, console: Console) -> None:  # noqa: C901, PLR0912
    """
    Display declaration details by ID.

    Args:
        declaration_id: Declaration ID to display.
        console: Rich console for output.
    """
    storage = Storage()
    declaration = storage.get_declaration(declaration_id)

    if not declaration:
        console.print(f"[red]Declaration '{declaration_id}' not found.[/red]")
        return

    # Show declaration header
    console.print(f"\n[bold]Declaration ID:[/bold] {declaration.declaration_id}")
    console.print(f"[bold]Type:[/bold] {declaration.type.value}")
    console.print(f"[bold]Status:[/bold] {declaration.status.value}")
    console.print(f"[bold]Period:[/bold] {declaration.period_start} to {declaration.period_end}")
    console.print(f"[bold]Created:[/bold] {declaration.created_at.strftime('%Y-%m-%d %H:%M:%S')}")

    if declaration.submitted_at:
        console.print(
            f"[bold]Submitted:[/bold] {declaration.submitted_at.strftime('%Y-%m-%d %H:%M:%S')}",
        )
    if declaration.paid_at:
        console.print(f"[bold]Paid:[/bold] {declaration.paid_at.strftime('%Y-%m-%d %H:%M:%S')}")
    if declaration.file_path:
        console.print(f"[bold]File:[/bold] {declaration.file_path}")

    # Show declaration data
    if declaration.report_data:
        console.print("\n[bold]Declaration Data:[/bold]")
        if declaration.type == DeclarationType.PPDG3R:
            # Gains: render table
            gains_entries = [e for e in declaration.report_data if isinstance(e, TaxReportEntry)]
            if gains_entries:
                table = render_declaration_table(gains_entries)
                console.print(table)
        else:
            # Income: show declaration fields
            for entry in declaration.report_data:
                if isinstance(entry, IncomeDeclarationEntry):
                    display_income_declaration(entry, console)

    # Show metadata
    if declaration.metadata:
        console.print("\n[bold]Metadata:[/bold]")
        metadata_table = Table(box=None, show_header=False)
        metadata_table.add_column("Key", justify="left", style="cyan")
        metadata_table.add_column("Value", justify="left")
        preferred_order = [
            "period_start",
            "period_end",
            "entry_count",
            "symbol",
            "income_type",
            "total_gain_rsd",
            "gross_income_rsd",
            "tax_base_rsd",
            "calculated_tax_rsd",
            "estimated_tax_rsd",
            "foreign_tax_paid_rsd",
            "assessed_tax_due_rsd",
            "tax_due_rsd",
        ]
        ordered_keys = [key for key in preferred_order if key in declaration.metadata]
        ordered_keys.extend(
            sorted(key for key in declaration.metadata if key not in set(ordered_keys)),
        )

        for key in ordered_keys:
            value = declaration.metadata[key]
            # Format value based on type (no thousands separators for easy copy-paste)
            if isinstance(value, int | float):
                formatted_value = f"{value:.2f}" if isinstance(value, float) else str(value)
            else:
                formatted_value = str(value)
            metadata_table.add_row(key, formatted_value)

        console.print(metadata_table)


def render_declaration_details_text(declaration_id: str) -> str:
    """Render declaration details exactly as CLI `show` output text."""
    buffer = StringIO()
    text_console = Console(file=buffer, force_terminal=False, color_system=None, width=120)
    show_declaration(declaration_id, text_console)
    return buffer.getvalue().strip()
