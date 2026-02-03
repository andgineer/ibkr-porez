"""Error handling utilities for user-friendly error messages."""


def get_user_friendly_error_message(exception: Exception) -> str:
    """
    Extract user-friendly error message from exception chain.

    Traverses the exception chain to find the most meaningful error message
    for the user, avoiding wrapper exceptions like RetryError.

    Args:
        exception: The exception to extract message from

    Returns:
        User-friendly error message string
    """
    # List of wrapper exception names to skip
    wrapper_names = ["RetryError", "Chained", "Wrapped"]

    # Start with the exception itself
    current: Exception | None = exception
    best_message = str(exception)

    # Traverse the exception chain to find the most meaningful message
    # Go to the deepest exception that is not a wrapper
    while current:
        msg = str(current)
        exception_type_name = type(current).__name__

        # Skip wrapper exceptions
        if any(wrapper in exception_type_name for wrapper in wrapper_names):
            # Move to the cause or context
            cause = getattr(current, "__cause__", None)
            context = getattr(current, "__context__", None)
            current = cause if cause is not None else context
            continue

        # Prefer specific error types (ValueError, RuntimeError, etc.)
        if isinstance(
            current,
            ValueError | RuntimeError | KeyError | AttributeError,
        ) or best_message == str(exception):
            best_message = msg

        # Move to the cause or context
        cause = getattr(current, "__cause__", None)
        context = getattr(current, "__context__", None)
        current = cause if cause is not None else context

    return best_message
