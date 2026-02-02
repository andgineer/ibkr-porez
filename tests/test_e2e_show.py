import allure
import pytest
from unittest.mock import patch
from click.testing import CliRunner
from ibkr_porez.main import ibkr_porez
from ibkr_porez.storage import Storage
from ibkr_porez.models import Transaction, TransactionType, Currency
from decimal import Decimal
from datetime import date


@pytest.fixture
def mock_user_data_dir(tmp_path):
    with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
        s = Storage()
        s._ensure_dirs()
        yield tmp_path


@pytest.fixture
def runner():
    return CliRunner()


@allure.epic("End-to-end")
@allure.feature("show")
class TestE2EShow:
    @pytest.fixture
    def setup_data(self, mock_user_data_dir):
        """Populate storage with some test data."""
        s = Storage()
        # AAPL: Buy 10 @ 100, Sell 5 @ 120 (Gain 20/share * 5 = 100 USD)
        # MSFT: Buy 10 @ 200, Sell 10 @ 220 (Gain 20/share * 10 = 200 USD)
        # Dividends: KO 50 USD
        txs = [
            Transaction(
                transaction_id="buy_aapl",
                date=date(2026, 1, 1),
                type=TransactionType.TRADE,
                symbol="AAPL",
                description="Buy",
                quantity=Decimal(10),
                price=Decimal(100),
                amount=Decimal("-1000"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="sell_aapl",
                date=date(2026, 1, 15),
                type=TransactionType.TRADE,
                symbol="AAPL",
                description="Sell",
                quantity=Decimal(-5),
                price=Decimal(120),
                amount=Decimal("600"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="buy_msft",
                date=date(2026, 2, 1),
                type=TransactionType.TRADE,
                symbol="MSFT",
                description="Buy",
                quantity=Decimal(10),
                price=Decimal(200),
                amount=Decimal("-2000"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="sell_msft",
                date=date(2026, 2, 10),
                type=TransactionType.TRADE,
                symbol="MSFT",
                description="Sell",
                quantity=Decimal(-10),
                price=Decimal(220),
                amount=Decimal("2200"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="div_ko",
                date=date(2026, 3, 15),
                type=TransactionType.DIVIDEND,
                symbol="KO",
                description="Dividend",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("50"),
                currency=Currency.USD,
            ),
        ]
        s.save_transactions(txs)
        return s

    @patch("ibkr_porez.show_statistics.NBSClient")
    def test_show_default_summary(self, mock_nbs_cls, runner, mock_user_data_dir, setup_data):
        """
        Scenario: Run `show` without arguments.
        Expect: Monthly summary table.
        """
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")  # 1 USD = 100 RSD

        result = runner.invoke(ibkr_porez, ["show"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        assert "Monthly Report Breakdown" in result.output

        # Verify Rows
        # Jan 2023: AAPL, Sales 1, P/L 100 USD * 100 = 10,000 RSD
        assert "2026-01" in result.output
        assert "AAPL" in result.output
        assert "10,000.00" in result.output

        # Feb 2023: MSFT, Sales 1, P/L 200 USD * 100 = 20,000 RSD
        assert "2026-02" in result.output
        assert "MSFT" in result.output
        assert "20,000.00" in result.output

        # Mar 2023: KO, Divs 50 USD * 100 = 5,000 RSD
        assert "2026-03" in result.output
        assert "KO" in result.output
        assert "5,000.00" in result.output

    @patch("ibkr_porez.show_statistics.NBSClient")
    def test_show_detailed_ticker(self, mock_nbs_cls, runner, mock_user_data_dir, setup_data):
        """
        Scenario: Run `show --ticker AAPL`.
        Expect: Detailed execution list for AAPL.
        """
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")

        result = runner.invoke(ibkr_porez, ["show", "--ticker", "AAPL"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        assert "Detailed Report: AAPL" in result.output

        # Should show sale date, quantity, prices
        # Sale Date 2023-01-15, Qty 5.00, Price 120.00
        # Date might be truncated in table output, so check for partial match
        assert "2026-01" in result.output or "2026-…" in result.output
        assert "5.00" in result.output
        assert "120.00" in result.output

        # Total P/L for this filter
        assert "Total P/L: 10,000.00 RSD" in result.output

        # Should NOT show MSFT
        assert "MSFT" not in result.output

    @patch("ibkr_porez.show_statistics.NBSClient")
    def test_show_detailed_month(self, mock_nbs_cls, runner, mock_user_data_dir, setup_data):
        """
        Scenario: Run `show --month 2023-02`.
        Expect: Summary filtered by month (or detailed? logic says if ticker OR simple filter... wait.
        Let's check logic: if ticker IS passed -> Detailed. If ONLY month -> Summary filtered?
        Code: `if show_detailed_list: ...` where `show_detailed_list = True if ticker else False`.
        So `show -m 2023-02` shows SUMMARY filtered (NOT Detailed).
        Wait, I should verify what I implemented.
        """
        # Re-reading main.py from previous task (Step 3688):
        # show_detailed_list = False
        # if ticker: show_detailed_list = True
        # So providing only month keeps it as SUMMARY.

        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")

        result = runner.invoke(ibkr_porez, ["show", "--month", "2026-02"], env={"COLUMNS": "200"})

        assert result.exit_code == 0
        assert "Monthly Report Breakdown" in result.output

        # Should show Feb data (MSFT)
        assert "2026-02" in result.output
        assert "MSFT" in result.output

        # Should NOT show Jan data (AAPL)
        assert "2026-01" not in result.output
        assert "AAPL" not in result.output

    @patch("ibkr_porez.show_statistics.NBSClient")
    def test_show_empty(self, mock_nbs_cls, runner, mock_user_data_dir):
        """Scenario: No transactions."""
        mock_nbs = mock_nbs_cls.return_value

        # Empty storage
        Storage()

        result = runner.invoke(ibkr_porez, ["show"])

        assert result.exit_code == 0
        assert "No transactions found" in result.output
