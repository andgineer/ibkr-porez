from __future__ import annotations

from ibkr_porez.error_handling import get_user_friendly_error_message


def test_get_user_friendly_error_message_prefers_deep_non_wrapper_cause() -> None:
    class RetryError(Exception):
        pass

    inner = ValueError("invalid token")
    outer = RetryError("retry failed")
    outer.__cause__ = inner

    assert get_user_friendly_error_message(outer) == "invalid token"


def test_get_user_friendly_error_message_traverses_cause_chain() -> None:
    try:
        raise KeyError("missing key")
    except KeyError as err:
        wrapped = RuntimeError("high-level failure")
        wrapped.__cause__ = err

    assert get_user_friendly_error_message(wrapped) == "'missing key'"


def test_get_user_friendly_error_message_falls_back_to_original_message() -> None:
    class RetryError(Exception):
        pass

    error = RetryError("retry failed")
    assert get_user_friendly_error_message(error) == "retry failed"
