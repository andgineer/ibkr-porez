"""Tests for report command date parameters logic."""

import allure
import pytest
from datetime import date, datetime
from unittest.mock import patch, MagicMock
from click.testing import CliRunner
from decimal import Decimal

from ibkr_porez.main import ibkr_porez
from ibkr_porez.storage import Storage


@pytest.fixture
def mock_user_data_dir(tmp_path):
    """Mock user data directory."""
    with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
        s = Storage()
        s._ensure_dirs()
        yield tmp_path


@pytest.fixture
def runner():
    """CLI runner fixture."""
    return CliRunner()


@allure.epic("Tax")
@allure.feature("PPDG-3R (gains)")
class TestReportDateParams:
    """Test report command date parameters logic and defaults."""

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_no_params_defaults_to_last_complete_half(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains without parameters defaults to last complete half-year."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(ibkr_porez, ["report", "--type", "gains"])

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output

        # Verify it uses last complete half-year
        now = datetime.now()
        if now.month < 7:
            expected_year = now.year - 1
            expected_half = 2
        else:
            expected_year = now.year
            expected_half = 1

        if expected_half == 1:
            expected_start = date(expected_year, 1, 1)
            expected_end = date(expected_year, 6, 30)
        else:
            expected_start = date(expected_year, 7, 1)
            expected_end = date(expected_year, 12, 31)

        assert str(expected_start) in result.output
        assert str(expected_end) in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_from_only_defaults_to_to_equals_from(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with only --start sets --end equal to --start."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--start", "2025-03-15"],
        )

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output
        # Both dates should be the same
        assert "(2025-03-15 to 2025-03-15)" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_to_only_uses_start_of_month_to_to(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with only --end uses start of current month to --end."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        now = datetime.now()
        start_of_month = date(now.year, now.month, 1)

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--end", "2025-03-15"],
        )

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output
        assert str(start_of_month) in result.output
        assert "2025-03-15" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_from_and_to_uses_specified_dates(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with --start and --end uses specified date range."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--start", "2025-01-15", "--end", "2025-02-20"],
        )

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output
        assert "(2025-01-15 to 2025-02-20)" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_half_takes_precedence_over_dates(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with --half and --start/--end, half takes precedence."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            [
                "report",
                "--type",
                "gains",
                "--half",
                "2023-1",
                "--start",
                "2025-01-15",
                "--end",
                "2025-02-20",
            ],
        )

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output
        # Should use half-year, not dates
        assert "(2023-01-01 to 2023-06-30)" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_invalid_date_format(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with invalid date format should show error."""
        mock_cfg_mgr.load_config.return_value = MagicMock()
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--start", "2025-01-15-extra"],
        )

        assert result.exit_code == 0
        assert "Invalid date format" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_from_after_to_error(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with --start after --end should show error."""
        mock_cfg_mgr.load_config.return_value = MagicMock()
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--start", "2025-06-01", "--end", "2025-01-01"],
        )

        assert result.exit_code == 0
        assert "Start date must be before or equal to end date" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_from_equals_to_valid(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with --start equal to --end should be valid."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--start", "2025-01-15", "--end", "2025-01-15"],
        )

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output
        assert "(2025-01-15 to 2025-01-15)" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_compact_date_format_invalid(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with compact date format (YYYYMMDD) should be invalid."""
        mock_cfg_mgr.load_config.return_value = MagicMock()
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--start", "20250115"],
        )

        assert result.exit_code == 0
        assert "Invalid date format" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_half_with_dash_format(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with --half in dash format (YYYY-H) should work."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--half", "2023-1"],
        )

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output
        assert "(2023-01-01 to 2023-06-30)" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_half_with_compact_format(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with --half in compact format (YYYYH) should work."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--half", "20232"],
        )

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output
        assert "(2023-07-01 to 2023-12-31)" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_default_type_is_gains(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report without --type defaults to gains."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(ibkr_porez, ["report"])

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output

    @patch("ibkr_porez.nbs.requests.get")
    @patch("ibkr_porez.report_base.NBSClient")
    @patch("ibkr_porez.report_base.config_manager")
    def test_report_gains_half_h2_covers_jul_dec(
        self, mock_cfg_mgr, mock_nbs_cls, mock_requests_get, runner, mock_user_data_dir
    ):
        """Report gains with --half H2 should cover July-December."""
        mock_cfg_mgr.load_config.return_value = MagicMock(
            personal_id="1234567890123",
            full_name="Test User",
            city_code="223",
        )
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = Decimal("117.0")

        result = runner.invoke(
            ibkr_porez,
            ["report", "--type", "gains", "--half", "2023-2"],
        )

        assert result.exit_code == 0
        assert "Generating PPDG-3R Report for" in result.output
        assert "(2023-07-01 to 2023-12-31)" in result.output
