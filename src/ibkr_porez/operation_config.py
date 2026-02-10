"""Operation for configuring IBKR and personal details."""

from pathlib import Path

import click
from rich.console import Console

from ibkr_porez.config import (
    UserConfig,
    config_manager,
    get_data_dir_change_warning,
    get_default_data_dir_path,
)


def _format_config_value(value: str | None, default: str, is_path: bool = False) -> str:
    """Format config value for display."""
    if value is None:
        return f"[dim](default: {default})[/dim]"
    if value == "":
        return "[dim](not set)[/dim]"
    if is_path:
        return value
    return value


def _get_default_data_dir() -> str:
    """Get default data directory path."""
    return str(get_default_data_dir_path())


def _get_default_output_folder() -> str:
    """Get default output folder path."""
    return str(Path.home() / "Downloads")


def _is_config_empty(config: UserConfig) -> bool:
    """Check if config is empty (new installation)."""
    return (
        not config.ibkr_token
        and not config.ibkr_query_id
        and not config.full_name
        and not config.address
    )


def _print_help_link(console: Console) -> None:
    """Print help link for IBKR Flex Token and Query ID."""
    console.print(
        "[dim]Need help getting your IBKR Flex Token and Query ID? "
        "See [link=https://andgineer.github.io/ibkr-porez/ibkr/#flex-web-service]"
        "documentation[/link].[/dim]\n",
    )


def _display_current_config(
    console: Console,
    config: UserConfig,
    default_data_dir: str,
    default_output_folder: str,
) -> None:
    """Display current configuration values."""
    console.print("[bold]Current Configuration:[/bold]")
    console.print(
        f"  IBKR Flex Token: {_format_config_value(config.ibkr_token, '(not set)')}",
    )
    console.print(
        f"  IBKR Query ID: {_format_config_value(config.ibkr_query_id, '(not set)')}",
    )
    personal_id_value = _format_config_value(config.personal_id, "(not set)")
    console.print(f"  Personal ID (JMBG): {personal_id_value}")
    console.print(f"  Full Name: {_format_config_value(config.full_name, '(not set)')}")
    console.print(f"  Address: {_format_config_value(config.address, '(not set)')}")
    console.print(f"  City Code: {_format_config_value(config.city_code, '223')}")
    console.print(f"  Phone: {_format_config_value(config.phone, '(not set)')}")
    console.print(f"  Email: {_format_config_value(config.email, '(not set)')}")
    data_dir_value = _format_config_value(
        config.data_dir,
        default_data_dir,
        is_path=True,
    )
    console.print(f"  Data Directory: {data_dir_value}")
    output_folder_value = _format_config_value(
        config.output_folder,
        default_output_folder,
        is_path=True,
    )
    console.print(f"  Output Folder: {output_folder_value}")
    console.print()


def _get_fields_to_update(
    console: Console,
    all_fields: list[str],
    is_empty: bool,
) -> list[str] | None:
    """Get list of fields to update from user input."""
    if is_empty:
        return all_fields

    console.print(
        "Select fields to update (comma-separated numbers, "
        "or 'all' for all fields, or Enter to skip):",
    )
    console.print("  1. IBKR Flex Token")
    console.print("  2. IBKR Query ID")
    console.print("  3. Personal ID (JMBG)")
    console.print("  4. Full Name")
    console.print("  5. Address")
    console.print("  6. City Code")
    console.print("  7. Phone")
    console.print("  8. Email")
    console.print("  9. Data Directory")
    console.print("  10. Output Folder")

    selection = click.prompt("\nSelection", default="", show_default=False).strip()

    if selection.lower() == "all":
        return all_fields
    if selection:
        try:
            indices = [int(x.strip()) - 1 for x in selection.split(",")]
            return [all_fields[i] for i in indices if 0 <= i < len(all_fields)]
        except ValueError:
            console.print(
                "[red]Invalid selection. Please enter numbers separated by commas.[/red]",
            )
            return None
    console.print("[yellow]No fields selected. Configuration unchanged.[/yellow]")
    return None


def _update_basic_fields(
    config_dict: dict,
    current_config: UserConfig,
    fields_to_update: list[str],
) -> None:
    """Update basic config fields (IBKR, personal info)."""
    if "ibkr_token" in fields_to_update:
        config_dict["ibkr_token"] = click.prompt(
            "IBKR Flex Token",
            default=current_config.ibkr_token,
        )

    if "ibkr_query_id" in fields_to_update:
        config_dict["ibkr_query_id"] = click.prompt(
            "IBKR Query ID",
            default=current_config.ibkr_query_id,
        )

    if "personal_id" in fields_to_update:
        config_dict["personal_id"] = click.prompt(
            "Personal Search ID (JMBG)",
            default=current_config.personal_id,
        )

    if "full_name" in fields_to_update:
        config_dict["full_name"] = click.prompt(
            "Full Name",
            default=current_config.full_name,
        )

    if "address" in fields_to_update:
        config_dict["address"] = click.prompt(
            "Address",
            default=current_config.address,
        )

    if "city_code" in fields_to_update:
        city_prompt = (
            "City/Municipality Code (Sifra opstine, "
            "e.g. 223 Novi Sad, 013 Novi Beograd. See portal)"
        )
        config_dict["city_code"] = click.prompt(
            city_prompt,
            default=current_config.city_code or "223",
        )

    if "phone" in fields_to_update:
        config_dict["phone"] = click.prompt(
            "Phone Number",
            default=current_config.phone,
        )

    if "email" in fields_to_update:
        config_dict["email"] = click.prompt(
            "Email",
            default=current_config.email,
        )


def _update_path_fields(
    config_dict: dict,
    current_config: UserConfig,
    fields_to_update: list[str],
    default_data_dir: str,
    default_output_folder: str,
) -> None:
    """Update path-related config fields (data_dir, output_folder)."""
    if "data_dir" in fields_to_update:
        default_data_dir_display = current_config.data_dir or default_data_dir
        data_dir_input = click.prompt(
            "Data Directory (absolute path to folder with transactions.json, "
            "default: ibkr-porez-data in app folder)",
            default=default_data_dir_display,
            show_default=True,
        )
        default_ibkr_porez_data = _get_default_data_dir()
        config_dict["data_dir"] = (
            None
            if data_dir_input.strip() == default_ibkr_porez_data
            else (data_dir_input.strip() if data_dir_input.strip() else None)
        )

    if "output_folder" in fields_to_update:
        default_output_folder_display = current_config.output_folder or default_output_folder
        output_folder_input = click.prompt(
            "Output Folder (absolute path to folder for saving files from sync, "
            "export, export-flex, report commands, default: Downloads)",
            default=default_output_folder_display,
            show_default=True,
        )
        config_dict["output_folder"] = (
            None
            if output_folder_input.strip() == default_output_folder
            else (output_folder_input.strip() if output_folder_input.strip() else None)
        )


def execute_config_command(console: Console) -> None:
    """Execute configuration command."""
    current_config = config_manager.load_config()
    is_empty = _is_config_empty(current_config)

    console.print("[bold blue]Configuration[/bold blue]")
    console.print(f"Config file location: {config_manager.config_path}\n")

    default_data_dir = _get_default_data_dir()
    default_output_folder = _get_default_output_folder()

    all_fields = [
        "ibkr_token",
        "ibkr_query_id",
        "personal_id",
        "full_name",
        "address",
        "city_code",
        "phone",
        "email",
        "data_dir",
        "output_folder",
    ]

    if is_empty:
        console.print("[bold]Initial Configuration Setup[/bold]")
        _print_help_link(console)
        fields_to_update = all_fields
    else:
        _display_current_config(console, current_config, default_data_dir, default_output_folder)
        _print_help_link(console)
        fields_to_update = _get_fields_to_update(console, all_fields, is_empty)
        if fields_to_update is None:
            return

    new_config_dict = current_config.model_dump()
    _update_basic_fields(new_config_dict, current_config, fields_to_update)
    _update_path_fields(
        new_config_dict,
        current_config,
        fields_to_update,
        default_data_dir,
        default_output_folder,
    )

    new_config = UserConfig(**new_config_dict)
    data_dir_warning = get_data_dir_change_warning(current_config, new_config)
    config_manager.save_config(new_config)
    console.print("\n[bold green]Configuration saved successfully![/bold green]")
    if data_dir_warning:
        console.print(f"[bold yellow]Warning:[/bold yellow] {data_dir_warning}")
