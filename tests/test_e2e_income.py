"""End-to-end tests for PP OPO (Capital Income) report generation."""

import allure
import pytest
from datetime import date
from decimal import Decimal
from unittest.mock import patch
from click.testing import CliRunner

from ibkr_porez.main import ibkr_porez
from ibkr_porez.storage import Storage
from ibkr_porez.models import Transaction, TransactionType, Currency


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
@allure.feature("PP OPO (income)")
class TestE2EIncome:
    @pytest.fixture
    def setup_data(self, mock_user_data_dir):
        """Populate storage with dividend transactions."""
        s = Storage()
        # Add dividend transactions
        txs = [
            Transaction(
                transaction_id="div_voo_1",
                date=date(2025, 12, 24),
                type=TransactionType.DIVIDEND,
                symbol="VOO",
                description="VOO CASH DIVIDEND",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("21.25"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="div_sgov_1",
                date=date(2025, 12, 24),
                type=TransactionType.DIVIDEND,
                symbol="SGOV",
                description="SGOV CASH DIVIDEND",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("87.55"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="div_voo_2",
                date=date(2025, 12, 25),
                type=TransactionType.DIVIDEND,
                symbol="VOO",
                description="VOO CASH DIVIDEND",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("10.00"),
                currency=Currency.USD,
            ),
            # Withholding tax transactions
            Transaction(
                transaction_id="tax_sgov",
                date=date(2025, 12, 24),
                type=TransactionType.WITHHOLDING_TAX,
                symbol="SGOV",
                description="SGOV US TAX",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("-26.27"),
                currency=Currency.USD,
            ),
            Transaction(
                transaction_id="tax_voo",
                date=date(2025, 12, 24),
                type=TransactionType.WITHHOLDING_TAX,
                symbol="VOO",
                description="VOO US TAX",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("-6.38"),
                currency=Currency.USD,
            ),
        ]
        s.save_transactions(txs)
        return s

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_income_basic(
        self,
        mock_config_manager,
        mock_nbs_cls,
        mock_requests_get,
        runner,
        mock_user_data_dir,
        setup_data,
    ):
        """Test basic income report generation."""
        mock_config = mock_config_manager.load_config.return_value
        mock_config.personal_id = "2403971060012"
        mock_config.full_name = "Test User"
        mock_config.address = "Test Address"
        mock_config.city_code = "223"
        mock_config.phone = "0600000000"
        mock_config.email = "test@example.com"

        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")  # 1 USD = 100 RSD

        with runner.isolated_filesystem():
            result = runner.invoke(
                ibkr_porez,
                ["report", "--type=income", "--start=2025-12-24", "--end=2025-12-25"],
            )

            assert result.exit_code == 0
        assert "Generating PP OPO Report" in result.output
        assert "declaration(s)" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_income_missing_tax_error(
        self,
        mock_config_manager,
        mock_nbs_cls,
        mock_requests_get,
        runner,
        mock_user_data_dir,
    ):
        """Test that error is raised when withholding tax is not found."""
        # Setup storage with dividend but NO withholding tax
        s = Storage()
        txs = [
            Transaction(
                transaction_id="div_voo_1",
                date=date(2025, 12, 24),
                type=TransactionType.DIVIDEND,
                symbol="VOO",
                description="VOO CASH DIVIDEND",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("21.25"),
                currency=Currency.USD,
            ),
        ]
        s.save_transactions(txs)

        mock_config = mock_config_manager.load_config.return_value
        mock_config.personal_id = "2403971060012"
        mock_config.full_name = "Test User"
        mock_config.address = "Test Address"
        mock_config.city_code = "223"
        mock_config.phone = "0600000000"
        mock_config.email = "test@example.com"

        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type=income", "--start=2025-12-24", "--end=2025-12-24"],
        )

        # Error should be displayed (but exit_code might be 0 if error is caught and displayed)
        assert "Withholding tax in IBKR" in result.output
        assert "not found" in result.output
        assert "--force" in result.output
        # The error is displayed but command might exit with 0
        # Check that no declarations were created
        assert (
            "declaration(s)" not in result.output or "Generated declaration(s)" not in result.output
        )

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_income_with_force(
        self,
        mock_config_manager,
        mock_nbs_cls,
        mock_requests_get,
        runner,
        mock_user_data_dir,
    ):
        """Test that --force flag allows creation with zero tax."""
        # Setup storage with dividend but NO withholding tax
        s = Storage()
        txs = [
            Transaction(
                transaction_id="div_voo_1",
                date=date(2025, 12, 24),
                type=TransactionType.DIVIDEND,
                symbol="VOO",
                description="VOO CASH DIVIDEND",
                quantity=Decimal(0),
                price=Decimal(0),
                amount=Decimal("21.25"),
                currency=Currency.USD,
            ),
        ]
        s.save_transactions(txs)

        mock_config = mock_config_manager.load_config.return_value
        mock_config.personal_id = "2403971060012"
        mock_config.full_name = "Test User"
        mock_config.address = "Test Address"
        mock_config.city_code = "223"
        mock_config.phone = "0600000000"
        mock_config.email = "test@example.com"

        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")

        with runner.isolated_filesystem():
            result = runner.invoke(
                ibkr_porez,
                ["report", "--type=income", "--start=2025-12-24", "--end=2025-12-24", "--force"],
            )

            assert result.exit_code == 0
        assert "WARNING: --force flag is set" in result.output
        assert "declaration(s)" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_income_no_data(
        self,
        mock_config_manager,
        mock_nbs_cls,
        mock_requests_get,
        runner,
        mock_user_data_dir,
    ):
        """Test income report with no income data."""
        mock_config = mock_config_manager.load_config.return_value
        mock_config.personal_id = "2403971060012"
        mock_config.full_name = "Test User"
        mock_config.address = "Test Address"
        mock_config.city_code = "223"
        mock_config.phone = "0600000000"
        mock_config.email = "test@example.com"

        mock_nbs = mock_nbs_cls.return_value

        # Empty storage
        Storage()

        with runner.isolated_filesystem():
            result = runner.invoke(
                ibkr_porez,
                ["report", "--type=income", "--start=2025-12-24", "--end=2025-12-25"],
            )

            assert result.exit_code == 0
        assert "No transactions found" in result.output or "No income" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_income_separate_by_date(
        self,
        mock_config_manager,
        mock_nbs_cls,
        mock_requests_get,
        runner,
        mock_user_data_dir,
        setup_data,
    ):
        """Test that income is separated by date."""
        mock_config = mock_config_manager.load_config.return_value
        mock_config.personal_id = "2403971060012"
        mock_config.full_name = "Test User"
        mock_config.address = "Test Address"
        mock_config.city_code = "223"
        mock_config.phone = "0600000000"
        mock_config.email = "test@example.com"

        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("100.0")

        with runner.isolated_filesystem():
            result = runner.invoke(
                ibkr_porez,
                ["report", "--type=income", "--start=2025-12-24", "--end=2025-12-25"],
            )

            assert result.exit_code == 0
            # Should create separate declarations for 2025-12-24 and 2025-12-25
            assert "2025-12-24" in result.output
            assert "2025-12-25" in result.output
