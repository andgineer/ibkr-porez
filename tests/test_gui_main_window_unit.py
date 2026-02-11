from __future__ import annotations

from datetime import date, datetime

import pytest

from ibkr_porez.gui.main_window import MainWindow
from ibkr_porez.models import Declaration, DeclarationStatus, DeclarationType


def _declaration(declaration_id: str, status: DeclarationStatus) -> Declaration:
    return Declaration(
        declaration_id=declaration_id,
        type=DeclarationType.PPDG3R,
        status=status,
        period_start=date(2026, 1, 1),
        period_end=date(2026, 1, 31),
        created_at=datetime(2026, 2, 1, 10, 0, 0),
    )


def test_visible_declaration_indices_respects_active_all_and_status_filters() -> None:
    window = MainWindow.__new__(MainWindow)
    window.declarations = [
        _declaration("d1", DeclarationStatus.DRAFT),
        _declaration("d2", DeclarationStatus.SUBMITTED),
        _declaration("d3", DeclarationStatus.PAID),
    ]

    window.status_filter = "Active"
    assert window._visible_declaration_indices() == [0, 1]

    window.status_filter = "All"
    assert window._visible_declaration_indices() == [0, 1, 2]

    window.status_filter = "Draft"
    assert window._visible_declaration_indices() == [0]

    window.status_filter = "Submitted"
    assert window._visible_declaration_indices() == [1]


@pytest.mark.parametrize(
    ("declaration_status", "target_status", "expected_method"),
    [
        (DeclarationStatus.DRAFT, "Submitted", "submit"),
        (DeclarationStatus.DRAFT, "Paid", "pay"),
        (DeclarationStatus.SUBMITTED, "Draft", "revert"),
    ],
)
def test_apply_status_to_ids_dispatches_to_declaration_manager(
    declaration_status: DeclarationStatus,
    target_status: str,
    expected_method: str,
) -> None:
    called: list[tuple[str, list[str]]] = []

    class FakeManager:
        def submit(self, ids: list[str]) -> None:
            called.append(("submit", ids))

        def pay(self, ids: list[str]) -> None:
            called.append(("pay", ids))

        def revert(self, ids: list[str], _target: DeclarationStatus) -> None:
            called.append(("revert", ids))

    window = MainWindow.__new__(MainWindow)
    window.declarations = [_declaration("d1", declaration_status)]
    window.declaration_manager = FakeManager()

    window.apply_status_to_ids(["d1"], target_status)

    assert called == [(expected_method, ["d1"])]


def test_apply_status_to_ids_raises_for_missing_declaration_id() -> None:
    class FakeManager:
        def submit(self, ids: list[str]) -> None:  # noqa: ARG002
            raise AssertionError("should not be called")

        def pay(self, ids: list[str]) -> None:  # noqa: ARG002
            raise AssertionError("should not be called")

        def revert(self, ids: list[str], _target: DeclarationStatus) -> None:  # noqa: ARG002
            raise AssertionError("should not be called")

    window = MainWindow.__new__(MainWindow)
    window.declarations = [_declaration("known", DeclarationStatus.DRAFT)]
    window.declaration_manager = FakeManager()

    with pytest.raises(ValueError, match="unknown"):
        window.apply_status_to_ids(["unknown"], "Submitted")


def test_apply_status_to_ids_raises_for_invalid_transition() -> None:
    class FakeManager:
        def submit(self, ids: list[str]) -> None:  # noqa: ARG002
            raise AssertionError("should not be called")

        def pay(self, ids: list[str]) -> None:  # noqa: ARG002
            raise AssertionError("should not be called")

        def revert(self, ids: list[str], _target: DeclarationStatus) -> None:  # noqa: ARG002
            raise AssertionError("should not be called")

    window = MainWindow.__new__(MainWindow)
    window.declarations = [_declaration("paid-decl", DeclarationStatus.PAID)]
    window.declaration_manager = FakeManager()

    with pytest.raises(ValueError, match="paid-decl"):
        window.apply_status_to_ids(["paid-decl"], "Submitted")
