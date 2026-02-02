"""Tests for complex withholding tax matching heuristics."""

import allure
import pytest
from datetime import date, timedelta
from decimal import Decimal
from unittest.mock import patch

import pandas as pd

from ibkr_porez.models import Currency, IncomeEntry, Transaction, TransactionType
from ibkr_porez.report_income import IncomeReportGenerator
from ibkr_porez.storage import Storage


@allure.epic("Tax")
@allure.feature("PP OPO (income)")
@allure.feature("Withholding Tax Matching")
class TestWithholdingTaxMatching:
    """Test complex heuristics for matching withholding tax to income."""

    @pytest.fixture
    def generator(self):
        """Create IncomeReportGenerator with mocked NBS."""
        with patch("ibkr_porez.report_income.NBSClient") as mock_nbs_cls:
            mock_nbs = mock_nbs_cls.return_value
            mock_nbs.get_rate.return_value = Decimal("100.0")  # 1 USD = 100 RSD
            gen = IncomeReportGenerator()
            gen.nbs = mock_nbs
            return gen

    def test_find_tax_in_subsequent_days(self, generator):
        """Test that tax can be found in subsequent days (up to 7 days)."""
        income_date = date(2025, 12, 24)
        tax_date = date(2025, 12, 26)  # 2 days later

        # Create income entry
        income_entry = IncomeEntry(
            date=income_date,
            symbol="VOO",
            amount=Decimal("100.00"),
            currency=Currency.USD,
            amount_rsd=Decimal("10000.00"),
            exchange_rate=Decimal("100.0"),
            income_type="dividend",
            description="VOO CASH DIVIDEND",
        )

        # Create withholding tax DataFrame (tax on different day)
        withholding_df = pd.DataFrame(
            [
                {
                    "date": tax_date,
                    "symbol": "VOO",
                    "currency": "USD",
                    "amount": Decimal("-15.00"),  # Negative
                    "description": "VOO US TAX",
                }
            ]
        )

        # Find tax
        tax_rsd = generator._find_withholding_tax(
            income_date=income_date,
            symbol="VOO",
            income_type="dividend",
            currency=Currency.USD,
            income_entries=[income_entry],
            withholding_df=withholding_df,
            max_days_offset=7,
        )

        # Should find tax: 15.00 USD * 100 = 1500.00 RSD
        assert tax_rsd == Decimal("1500.00")

    def test_find_tax_not_found_beyond_range(self, generator):
        """Test that tax is not found if beyond 7 days range."""
        income_date = date(2025, 12, 24)
        tax_date = date(2026, 1, 2)  # 9 days later (beyond 7 days)

        income_entry = IncomeEntry(
            date=income_date,
            symbol="VOO",
            amount=Decimal("100.00"),
            currency=Currency.USD,
            amount_rsd=Decimal("10000.00"),
            exchange_rate=Decimal("100.0"),
            income_type="dividend",
            description="VOO CASH DIVIDEND",
        )

        withholding_df = pd.DataFrame(
            [
                {
                    "date": tax_date,
                    "symbol": "VOO",
                    "currency": "USD",
                    "amount": Decimal("-15.00"),
                    "description": "VOO US TAX",
                }
            ]
        )

        tax_rsd = generator._find_withholding_tax(
            income_date=income_date,
            symbol="VOO",
            income_type="dividend",
            currency=Currency.USD,
            income_entries=[income_entry],
            withholding_df=withholding_df,
            max_days_offset=7,
        )

        # Should not find tax (beyond range)
        assert tax_rsd == Decimal("0.00")

    def test_match_dividend_by_entity_name_isin(self, generator):
        """Test that dividends are matched by entity_name/ISIN from description."""
        income_date = date(2025, 12, 24)

        # Income with description containing entity_name and ISIN
        income_entry = IncomeEntry(
            date=income_date,
            symbol="VOO",
            amount=Decimal("100.00"),
            currency=Currency.USD,
            amount_rsd=Decimal("10000.00"),
            exchange_rate=Decimal("100.0"),
            income_type="dividend",
            description="VOO (US9229083632) CASH DIVIDEND",  # Contains ISIN
        )

        # Tax with matching entity_name/ISIN but different symbol
        withholding_df = pd.DataFrame(
            [
                {
                    "date": income_date,
                    "symbol": "DIFFERENT",  # Different symbol
                    "currency": "USD",
                    "amount": Decimal("-15.00"),
                    "description": "VOO (US9229083632) US TAX",  # Same entity_name/ISIN
                }
            ]
        )

        tax_rsd = generator._find_withholding_tax(
            income_date=income_date,
            symbol="VOO",
            income_type="dividend",
            currency=Currency.USD,
            income_entries=[income_entry],
            withholding_df=withholding_df,
            max_days_offset=7,
        )

        # Should match by entity_name/ISIN even though symbol differs
        assert tax_rsd == Decimal("1500.00")

    def test_match_dividend_fallback_to_symbol(self, generator):
        """Test that dividends fallback to symbol match if entity_name/ISIN not found."""
        income_date = date(2025, 12, 24)

        # Income without entity_name/ISIN in description
        income_entry = IncomeEntry(
            date=income_date,
            symbol="VOO",
            amount=Decimal("100.00"),
            currency=Currency.USD,
            amount_rsd=Decimal("10000.00"),
            exchange_rate=Decimal("100.0"),
            income_type="dividend",
            description="VOO CASH DIVIDEND",  # No ISIN
        )

        # Tax with matching symbol
        withholding_df = pd.DataFrame(
            [
                {
                    "date": income_date,
                    "symbol": "VOO",  # Matching symbol
                    "currency": "USD",
                    "amount": Decimal("-15.00"),
                    "description": "VOO US TAX",
                }
            ]
        )

        tax_rsd = generator._find_withholding_tax(
            income_date=income_date,
            symbol="VOO",
            income_type="dividend",
            currency=Currency.USD,
            income_entries=[income_entry],
            withholding_df=withholding_df,
            max_days_offset=7,
        )

        # Should match by symbol (fallback)
        assert tax_rsd == Decimal("1500.00")

    def test_match_interest_by_currency(self, generator):
        """Test that interest is matched by currency, not symbol."""
        income_date = date(2025, 12, 24)

        # Interest entry with empty symbol
        income_entry = IncomeEntry(
            date=income_date,
            symbol="",  # Empty symbol for interest
            amount=Decimal("100.00"),
            currency=Currency.USD,
            amount_rsd=Decimal("10000.00"),
            exchange_rate=Decimal("100.0"),
            income_type="coupon",
            description="USD Credit Interest for Account",
        )

        # Tax with different symbol but same currency
        withholding_df = pd.DataFrame(
            [
                {
                    "date": income_date,
                    "symbol": "DIFFERENT",  # Different symbol
                    "currency": "USD",  # Same currency
                    "amount": Decimal("-15.00"),
                    "description": "USD Interest Tax",
                }
            ]
        )

        tax_rsd = generator._find_withholding_tax(
            income_date=income_date,
            symbol="",  # Empty symbol
            income_type="coupon",
            currency=Currency.USD,
            income_entries=[income_entry],
            withholding_df=withholding_df,
            max_days_offset=7,
        )

        # Should match by currency (not symbol)
        assert tax_rsd == Decimal("1500.00")

    def test_interest_grouping_by_currency(self, generator, tmp_path):
        """Test that interest transactions are grouped by currency, not symbol."""
        with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
            # Create fresh storage
            storage = Storage()
            storage._ensure_dirs()

            # Add multiple interest transactions in same currency but different symbols
            transactions = [
                Transaction(
                    transaction_id="int1",
                    date=date(2025, 12, 24),
                    type=TransactionType.INTEREST,
                    symbol="",  # Empty symbol
                    description="USD Credit Interest",
                    quantity=Decimal(0),
                    price=Decimal(0),
                    amount=Decimal("100.00"),
                    currency=Currency.USD,
                ),
                Transaction(
                    transaction_id="int2",
                    date=date(2025, 12, 24),
                    type=TransactionType.INTEREST,
                    symbol="CASH",  # Different symbol
                    description="USD Debit Interest",
                    quantity=Decimal(0),
                    price=Decimal(0),
                    amount=Decimal("50.00"),
                    currency=Currency.USD,
                ),
                Transaction(
                    transaction_id="tax1",
                    date=date(2025, 12, 24),
                    type=TransactionType.WITHHOLDING_TAX,
                    symbol="",  # Empty symbol
                    description="USD Interest Tax",
                    quantity=Decimal(0),
                    price=Decimal(0),
                    amount=Decimal("-15.00"),
                    currency=Currency.USD,
                ),
            ]

            storage.save_transactions(transactions)

            # Create new generator with fresh storage
            with patch("ibkr_porez.report_income.NBSClient") as mock_nbs_cls:
                mock_nbs = mock_nbs_cls.return_value
                mock_nbs.get_rate.return_value = Decimal("100.0")
                gen = IncomeReportGenerator()
                gen.nbs = mock_nbs

                # Generate reports
                results = list(
                    gen.generate(
                        start_date=date(2025, 12, 24),
                        end_date=date(2025, 12, 24),
                    )
                )

                # Should create ONE declaration (grouped by currency, not symbol)
                assert len(results) == 1

                # Check that both interest amounts are included (100 + 50 = 150)
                result = results[0]
                declaration_entry = result[1][0]
                # Total should be sum of both: 150.00 USD * 100 = 15000.00 RSD
                assert declaration_entry.bruto_prihod == Decimal("15000.00")

    def test_multiple_taxes_summed(self, generator):
        """Test that multiple taxes in range are summed."""
        income_date = date(2025, 12, 24)

        income_entry = IncomeEntry(
            date=income_date,
            symbol="VOO",
            amount=Decimal("100.00"),
            currency=Currency.USD,
            amount_rsd=Decimal("10000.00"),
            exchange_rate=Decimal("100.0"),
            income_type="dividend",
            description="VOO CASH DIVIDEND",
        )

        # Multiple taxes on different days
        withholding_df = pd.DataFrame(
            [
                {
                    "date": income_date,
                    "symbol": "VOO",
                    "currency": "USD",
                    "amount": Decimal("-10.00"),
                    "description": "VOO US TAX 1",
                },
                {
                    "date": income_date + timedelta(days=1),
                    "symbol": "VOO",
                    "currency": "USD",
                    "amount": Decimal("-5.00"),
                    "description": "VOO US TAX 2",
                },
            ]
        )

        tax_rsd = generator._find_withholding_tax(
            income_date=income_date,
            symbol="VOO",
            income_type="dividend",
            currency=Currency.USD,
            income_entries=[income_entry],
            withholding_df=withholding_df,
            max_days_offset=7,
        )

        # Should sum both taxes: (10 + 5) * 100 = 1500.00 RSD
        assert tax_rsd == Decimal("1500.00")

    def test_parse_entity_from_description(self, generator):
        """Test parsing entity_name and ISIN from description."""
        # Test with ISIN
        entity_name, entity_isin = generator._parse_entity_from_description(
            "VOO (US9229083632) CASH DIVIDEND"
        )
        assert entity_name == "VOO"
        assert entity_isin == "US9229083632"

        # Test without ISIN
        entity_name, entity_isin = generator._parse_entity_from_description("VOO CASH DIVIDEND")
        assert entity_name is None
        assert entity_isin is None

        # Test with different format
        entity_name, entity_isin = generator._parse_entity_from_description(
            "SGOV(US46436E7186) DIVIDEND"
        )
        assert entity_name == "SGOV"
        assert entity_isin == "US46436E7186"
