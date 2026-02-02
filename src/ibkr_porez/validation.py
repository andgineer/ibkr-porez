"""Validation error handling utilities."""

from pydantic import ValidationError


def format_validation_error(error: ValidationError) -> str:
    """
    Format Pydantic ValidationError into a clean, user-friendly message.

    Extracts error messages from ValidationError and combines multiple errors.

    Args:
        error: Pydantic ValidationError instance.

    Returns:
        str: Clean error message.
    """
    error_messages = []
    for err in error.errors():
        msg = err.get("msg", "")
        if msg:
            error_messages.append(msg)

    # Join all error messages, or fallback to string representation
    if error_messages:
        return " ".join(error_messages)

    # Fallback: use string representation
    return str(error)


def handle_validation_error(error: ValidationError, console_instance) -> None:
    """
    Handle ValidationError by printing a clean error message to console.

    Args:
        error: Pydantic ValidationError instance.
        console_instance: Rich Console instance to print the error.
    """
    error_msg = format_validation_error(error)
    console_instance.print(f"[red]{error_msg}[/red]")
