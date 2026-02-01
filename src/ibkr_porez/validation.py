"""Validation error handling utilities."""

from pydantic import ValidationError


def format_validation_error(error: ValidationError) -> str:
    """
    Format Pydantic ValidationError into a clean, user-friendly message.

    Removes Pydantic documentation links and extracts the actual error messages.
    Handles multiple validation errors by combining them.

    Args:
        error: Pydantic ValidationError instance.

    Returns:
        str: Clean error message without Pydantic documentation links.
    """
    error_messages = []
    for err in error.errors():
        msg = err.get("msg", "")
        # Remove Pydantic documentation link if present
        if "For further information visit" in msg:
            msg = msg.split("For further information visit")[0].strip()
        if msg:
            error_messages.append(msg)

    # Join all error messages, or fallback to string representation
    if error_messages:
        return " ".join(error_messages)

    # Fallback: use string representation and clean it
    error_msg = str(error)
    if "For further information visit" in error_msg:
        error_msg = error_msg.split("For further information visit")[0].strip()

    # Extract just the actual error message (skip "validation error" prefix)
    lines = error_msg.split("\n")
    for line in lines:
        if "Value error," in line:
            error_msg = line.split("Value error,")[-1].strip()
            break

    return error_msg


def handle_validation_error(error: ValidationError, console_instance) -> None:
    """
    Handle ValidationError by printing a clean error message to console.

    Args:
        error: Pydantic ValidationError instance.
        console_instance: Rich Console instance to print the error.
    """
    error_msg = format_validation_error(error)
    console_instance.print(f"[red]{error_msg}[/red]")
