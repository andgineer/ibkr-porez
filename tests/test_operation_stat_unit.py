from __future__ import annotations

from datetime import date, datetime
from types import SimpleNamespace

import allure
import pandas as pd

import ibkr_porez.operation_stat as operation_stat_module
from ibkr_porez.operation_stat import ShowStatistics


def _stat_instance() -> ShowStatistics:
    return ShowStatistics.__new__(ShowStatistics)


@allure.epic("Tax")
@allure.feature("Show Statistics")
class TestOperationStatUnit:
    def test_resolve_month_year_with_month_only_uses_latest_sales_year(self) -> None:
        sales_entries = [
            SimpleNamespace(sale_date=date(2024, 2, 10)),
            SimpleNamespace(sale_date=date(2026, 2, 2)),
        ]
        transactions = pd.DataFrame({"type": ["DIVIDEND"], "date": ["2025-02-03"]})

        result = ShowStatistics.resolve_month_year(
            _stat_instance(),
            month=2,
            year=None,
            sales_entries=sales_entries,
            df_transactions=transactions,
        )

        assert result == (2026, 2)

    def test_resolve_month_year_with_month_only_uses_latest_dividend_year_when_no_sales(
        self,
    ) -> None:
        sales_entries: list[object] = []
        transactions = pd.DataFrame(
            {
                "type": ["DIVIDEND", "DIVIDEND", "TRADE"],
                "date": ["2023-07-01", "2025-07-20", "2026-07-11"],
            },
        )

        result = ShowStatistics.resolve_month_year(
            _stat_instance(),
            month=7,
            year=None,
            sales_entries=sales_entries,
            df_transactions=transactions,
        )

        assert result == (2025, 7)

    def test_resolve_month_year_with_month_only_falls_back_to_current_year(
        self,
        monkeypatch,
    ) -> None:
        class FakeDatetime:
            @staticmethod
            def now() -> datetime:
                return datetime(2030, 5, 15)

        monkeypatch.setattr(operation_stat_module, "datetime", FakeDatetime)

        result = ShowStatistics.resolve_month_year(
            _stat_instance(),
            month=9,
            year=None,
            sales_entries=[],
            df_transactions=pd.DataFrame(),
        )

        assert result == (2030, 9)
