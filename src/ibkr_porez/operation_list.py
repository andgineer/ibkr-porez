"""List declarations command implementation."""

from rich.table import Table

from ibkr_porez.models import DECLARATION_STATUS_SCOPES, DeclarationStatus
from ibkr_porez.storage import Storage


class ListDeclarations:
    """Controller for list command."""

    def __init__(self):
        self.storage = Storage()

    def generate(
        self,
        show_all: bool = False,
        status: DeclarationStatus | None = None,
        ids_only: bool = False,
    ) -> Table | list[str]:
        """
        Generate declarations list table or IDs only.

        Args:
            show_all: If True, show all declarations regardless of status
            status: Filter by specific status
            ids_only: If True, return list of IDs instead of table

        Returns:
            Table or list[str]: Rich table with declarations or list of IDs
        """
        # Determine filter
        if show_all:
            declarations = self.storage.get_declarations()
        elif status:
            declarations = self.storage.get_declarations(status=status)
        else:
            # Default: show active declarations
            declarations = self.storage.get_declarations()
            declarations = [
                d for d in declarations if d.status in DECLARATION_STATUS_SCOPES["Active"]
            ]

        # Sort by created_at descending
        declarations.sort(key=lambda d: d.created_at, reverse=True)

        if ids_only:
            return [decl.declaration_id for decl in declarations]

        # Create table
        table = Table(title="Declarations")
        table.add_column("ID", style="cyan", no_wrap=True)
        table.add_column("Type", style="magenta")
        table.add_column("Period", style="green")
        table.add_column("Tax", style="bright_white", justify="right")
        table.add_column("Status", style="yellow")
        table.add_column("Created", style="blue")
        table.add_column("Attachments", style="dim")

        for decl in declarations:
            period_str = decl.display_period()
            status_str = decl.status.value
            created_str = decl.created_at.strftime("%Y-%m-%d %H:%M")
            files_count = len(decl.attached_files) if decl.attached_files else 0
            files_str = f"{files_count} attachments" if files_count > 0 else ""

            table.add_row(
                decl.declaration_id,
                decl.display_type(),
                period_str,
                decl.display_tax(),
                status_str,
                created_str,
                files_str,
            )

        return table
