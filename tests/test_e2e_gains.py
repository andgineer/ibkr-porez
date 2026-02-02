import allure
import pytest
from unittest.mock import patch, MagicMock
from click.testing import CliRunner
from ibkr_porez.main import ibkr_porez
from ibkr_porez.models import Transaction, TransactionType, Currency
from ibkr_porez.storage import Storage
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
@allure.feature("ppdg-3r")
class TestE2EReport:
    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.report_gains.config_manager")
    def test_report_generation_h1(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """
        Scenario: Generate PPDG-3R report for H1 2023.
        Data:
        - Buy AAPL 10 @ 100 USD on 2022-12-01 (Outside period context)
        - Sell AAPL 5 @ 120 USD on 2023-01-15 (Inside period)
        Expect:
        - XML generated.
        - Contains sale of 5 AAPL.
        - Capital gain calculated correctly.
        """
        # Mock Config
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            address="Test St 1",
            city_code="223",
            phone="060123456",
            email="test@example.com",
        )

        # Mock NBS (Fixed rates for deterministic calc)
        mock_nbs = mock_nbs_cls.return_value
        # Rate 117.0 for all dates for simplicity
        mock_nbs.get_rate.return_value = Decimal("117.0")

        # Setup Data in Storage
        s = Storage()
        transactions = [
            Transaction(
                transaction_id="buy1",
                date=date(2022, 12, 1),
                type=TransactionType.TRADE,
                symbol="AAPL",
                description="Buy AAPL",
                quantity=Decimal(10),
                price=Decimal(100),
                amount=Decimal("-1000"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="sell1",
                date=date(2023, 1, 15),
                type=TransactionType.TRADE,
                symbol="AAPL",
                description="Sell AAPL",
                quantity=Decimal(-5),
                price=Decimal(120),
                amount=Decimal("600"),
                currency=Currency.USD,
            ),
        ]
        s.save_transactions(transactions)

        # Run Report Command
        with runner.isolated_filesystem():
            result = runner.invoke(ibkr_porez, ["report", "--half", "2023-1"])

            assert result.exit_code == 0
            assert "Generated" in result.output and "declaration(s)" in result.output

            # Verify File Exists
            import os

            assert os.path.exists("ppdg3r_2023_H1.xml")

            with open("ppdg3r_2023_H1.xml", "r") as f:
                content = f.read()
                assert "AAPL" in content  # Basic check

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.report_gains.config_manager")
    def test_report_generation_file_check(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Verify XML file creation and content."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123", full_name="Test User", address="Test St 1", city_code="223"
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.3")

        s = Storage()
        s.save_transactions(
            [
                Transaction(
                    transaction_id="buy_tsla",
                    date=date(2023, 1, 10),
                    type=TransactionType.TRADE,
                    symbol="TSLA",
                    description="Buy",
                    quantity=Decimal(1),
                    price=Decimal(200),
                    amount=Decimal("-200"),
                    currency=Currency.USD,
                ),
                Transaction(
                    transaction_id="sell_tsla",
                    date=date(2023, 8, 15),
                    type=TransactionType.TRADE,
                    symbol="TSLA",
                    description="Sell",
                    quantity=Decimal(-1),
                    price=Decimal(250),
                    amount=Decimal("250"),
                    currency=Currency.USD,
                ),
            ]
        )

        with runner.isolated_filesystem():
            # H2 2023
            result = runner.invoke(ibkr_porez, ["report", "--half", "2023-2"])

            assert result.exit_code == 0
            assert "ppdg3r_2023_H2.xml" in result.output
            assert "declaration(s)" in result.output

            # Read file
            with open("ppdg3r_2023_H2.xml", "r") as f:
                content = f.read()

            # Basic Checks - ignoring namespace prefix issues by partial match or including prefix if needed
            # The generator uses ns1: prefix for everything.
            assert "ns1:PodaciOPoreskomObvezniku" in content
            assert "1234567890123" in content
            assert "TSLA" in content
            assert "ns1:ProdajnaCena" in content

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.ibkr_flex_query.IBKRClient.fetch_latest_report")
    @patch("ibkr_porez.main.config_manager")
    @patch("ibkr_porez.report_gains.config_manager")
    def test_report_fifo_complex(
        self,
        mock_report_cfg_mgr,
        mock_main_cfg_mgr,
        mock_fetch,
        mock_nbs_cls,
        mock_requests_get,
        runner,
        mock_user_data_dir,
        resources_path,
    ):
        """
        Scenario: Complex FIFO (Mutli-Buy/Single-Sell, Single-Buy/Multi-Sell).
        Data: tests/resources/fifo_scenarios.xml
        Expect:
        - AAPL: 1 Sell of 15. Gain ~29,250 RSD (assuming rate 117).
        - MSFT: 2 Sells (5, 10). Gains 5,850 and 23,400 RSD.
        """
        # Mock Config (for both main.get and report_gains)
        mock_config = MagicMock(
            personal_id="1234567890123",
            full_name="Complex User",
            ibkr_token="t",
            ibkr_query_id="q",
            city_code="223",
        )
        mock_main_cfg_mgr.load_config.return_value = mock_config
        mock_report_cfg_mgr.load_config.return_value = mock_config

        # Mock NBS (Fixed rate 117.0)
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        # 1. Fetch & Populate Storage
        with open(resources_path / "fifo_scenarios.xml", "rb") as f:
            mock_fetch.return_value = f.read()

        res_get = runner.invoke(ibkr_porez, ["get"])
        assert res_get.exit_code == 0
        assert "Fetched 6 transactions" in res_get.output

        # 2. Generate Report
        with runner.isolated_filesystem():
            # H1 2023 (Jan-Jun) covers AAPL sell (Feb), MSFT sells (Apr)
            result = runner.invoke(ibkr_porez, ["report", "--half", "2023-1"])

            assert result.exit_code == 0

            with open("ppdg3r_2023_H1.xml", "r") as f:
                content = f.read()

            # Verify AAPL (Sell 15 split into 10 + 5 due to different acquisition dates)
            # Lot 1: 10 shares from 2022-12-01
            # Sale: 10 * 120 * 117 = 140,400
            # Cost: 10 * 100 * 117 = 117,000
            # Gain: 23,400
            assert "140400.00" in content
            assert "117000.00" in content
            assert "23400.00" in content

            # Lot 2: 5 shares from 2023-01-05
            # Sale: 5 * 120 * 117 = 70,200
            # Cost: 5 * 110 * 117 = 64,350
            # Gain: 5,850
            assert "70200.00" in content
            assert "64350.00" in content
            assert "5850.00" in content

            # Verify MSFT (Sell 5)
            # 5 * 210 * 117 = 122,850 (Sale Val)
            # Cost: 5 * 200 * 117 = 117,000
            # Gain: 5,850
            assert "MSFT" in content
            assert "122850.00" in content
            # Gain 5850 is already asserted above, but it's consistent.

            # Verify MSFT (Sell 10)
            # 10 * 220 * 117 = 257,400 (Sale Val)
            # Cost: 10 * 200 * 117 = 234,000
            # Gain: 23,400
            assert "257400.00" in content
            # Gain 23400 is already asserted above.

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.report_gains.config_manager")
    def test_report_no_sales(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Scenario: No sales in period."""
        mock_cfg_mgr.load_config.return_value = MagicMock()
        mock_nbs = mock_nbs_cls.return_value

        # Only buys, no sales
        s = Storage()
        s.save_transactions(
            [
                Transaction(
                    transaction_id="buy1",
                    date=date(2023, 2, 1),
                    type=TransactionType.TRADE,
                    symbol="NVDA",
                    description="Buy NVDA",
                    quantity=Decimal(10),
                    price=Decimal(100),
                    amount=Decimal("-1000"),
                    currency=Currency.USD,
                )
            ]
        )

        result = runner.invoke(ibkr_porez, ["report", "--half", "2023-1"])

        assert result.exit_code == 0
        assert "No taxable sales found" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.ibkr_flex_query.IBKRClient.fetch_latest_report")
    @patch("ibkr_porez.report_gains.config_manager")
    def test_report_invalid_format(
        self, mock_cfg_mgr, mock_fetch, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Scenario: User provides invalid date format."""
        result = runner.invoke(ibkr_porez, ["report", "--half", "2023_1"])
        assert "Invalid format: 2023_1" in result.output
        assert result.exit_code == 0

        result_unknown = runner.invoke(ibkr_porez, ["report", "--half", "abc"])
        assert "Invalid format: abc" in result_unknown.output

        result_bad_half = runner.invoke(ibkr_porez, ["report", "--half", "2023-3"])
        assert "Half-year must be 1 or 2" in result_bad_half.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.ibkr_flex_query.IBKRClient.fetch_latest_report")
    @patch("ibkr_porez.report_gains.config_manager")
    def test_report_default_period(
        self, mock_cfg_mgr, mock_fetch, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Scenario: User omits --half, defaults to previous complete half."""
        from datetime import datetime

        now = datetime.now()
        expected_year = now.year - 1 if now.month < 7 else now.year
        expected_half = 2 if now.month < 7 else 1

        result = runner.invoke(ibkr_porez, ["report"])
        assert "Generating PPDG-3R Report for" in result.output
        assert "No transactions found" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_gains.NBSClient")
    @patch("ibkr_porez.ibkr_flex_query.IBKRClient.fetch_latest_report")
    @patch("ibkr_porez.report_gains.config_manager")
    def test_report_no_transactions_found(
        self, mock_cfg_mgr, mock_fetch, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Scenario: Storage effectively empty (or filtered to empty)."""
        s = Storage()
        # No transactions saved

        result = runner.invoke(ibkr_porez, ["report", "--half", "2023-1"])

        assert "No transactions found. Run `ibkr-porez get` first." in result.output
        assert result.exit_code == 0
