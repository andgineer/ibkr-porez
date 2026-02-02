"""Tests for report parameter validation."""

import allure
import pytest
from datetime import date, datetime
from pydantic import ValidationError

from ibkr_porez.report_params import ReportParams, ReportType


@allure.epic("Tax")
@allure.feature("PPDG-3R (gains)")
class TestReportParams:
    """Test ReportParams validation and period calculation."""

    def test_default_type_is_gains(self):
        """Default report type should be GAINS."""
        params = ReportParams.model_validate({})
        assert params.type == ReportType.GAINS

    def test_type_income(self):
        """Can set type to INCOME."""
        params = ReportParams.model_validate({"type": "income"})
        assert params.type == ReportType.INCOME

    def test_type_gains(self):
        """Can set type to GAINS."""
        params = ReportParams.model_validate({"type": "gains"})
        assert params.type == ReportType.GAINS

    def test_valid_date_format(self):
        """Valid date format YYYY-MM-DD should be accepted."""
        params = ReportParams.model_validate({"start": "2025-01-15"})
        assert params.start_date == "2025-01-15"

    def test_invalid_date_format(self):
        """Invalid date format should raise ValidationError."""
        with pytest.raises(ValidationError) as exc_info:
            ReportParams.model_validate({"start": "2025-01-15-extra"})

        error_msg = str(exc_info.value)
        assert "Invalid date format" in error_msg

    def test_invalid_date_format_compact(self):
        """Invalid compact date format should raise ValidationError."""
        with pytest.raises(ValidationError) as exc_info:
            ReportParams.model_validate({"start": "20250115"})

        error_msg = str(exc_info.value)
        assert "Invalid date format" in error_msg

    def test_valid_half_format_dash(self):
        """Valid half-year format with dash (YYYY-H) should be accepted."""
        params = ReportParams.model_validate({"half": "2023-2"})
        assert params.half == "2023-2"

    def test_valid_half_format_compact(self):
        """Valid half-year format compact (YYYYH) should be accepted."""
        params = ReportParams.model_validate({"half": "20232"})
        assert params.half == "20232"

    def test_invalid_half_format(self):
        """Invalid half-year format should raise ValidationError."""
        with pytest.raises(ValidationError) as exc_info:
            ReportParams.model_validate({"half": "2023_1"})

        error_msg = str(exc_info.value)
        assert "Invalid format" in error_msg

    def test_invalid_half_number(self):
        """Half-year must be 1 or 2."""
        with pytest.raises(ValidationError) as exc_info:
            ReportParams.model_validate({"half": "2023-3"})

        error_msg = str(exc_info.value)
        assert "Half-year must be 1 or 2" in error_msg

    def test_date_range_start_after_end(self):
        """Start date must be before or equal to end date."""
        with pytest.raises(ValidationError) as exc_info:
            ReportParams.model_validate(
                {"start": "2025-06-01", "end": "2025-01-01"},
            )

        error_msg = str(exc_info.value)
        assert "Start date must be before or equal to end date" in error_msg

    def test_date_range_start_equals_end(self):
        """Start date can equal end date."""
        params = ReportParams.model_validate(
            {"start": "2025-01-01", "end": "2025-01-01"},
        )
        assert params.start_date == "2025-01-01"
        assert params.end_date == "2025-01-01"

    def test_start_without_end_defaults_to_start(self):
        """If --start is provided and --end is empty, --end defaults to --start."""
        params = ReportParams.model_validate({"start": "2025-01-15"})
        assert params.start_date == "2025-01-15"
        assert params.end_date == "2025-01-15"

    def test_get_period_gains_with_half_h1(self):
        """get_period for GAINS with half-year H1 returns Jan-Jun."""
        params = ReportParams.model_validate({"type": "gains", "half": "2023-1"})
        start, end = params.get_period()
        assert start == date(2023, 1, 1)
        assert end == date(2023, 6, 30)

    def test_get_period_gains_with_half_h2(self):
        """get_period for GAINS with half-year H2 returns Jul-Dec."""
        params = ReportParams.model_validate({"type": "gains", "half": "2023-2"})
        start, end = params.get_period()
        assert start == date(2023, 7, 1)
        assert end == date(2023, 12, 31)

    def test_get_period_gains_with_half_compact_format(self):
        """get_period for GAINS with compact half-year format works."""
        params = ReportParams.model_validate({"type": "gains", "half": "20232"})
        start, end = params.get_period()
        assert start == date(2023, 7, 1)
        assert end == date(2023, 12, 31)

    def test_get_period_gains_with_dates(self):
        """get_period for GAINS with date range returns those dates."""
        params = ReportParams.model_validate(
            {"type": "gains", "start": "2025-01-15", "end": "2025-02-20"},
        )
        start, end = params.get_period()
        assert start == date(2025, 1, 15)
        assert end == date(2025, 2, 20)

    def test_get_period_gains_default_last_complete_half_year(self):
        """get_period for GAINS defaults to last complete half-year."""
        params = ReportParams.model_validate({"type": "gains"})
        start, end = params.get_period()

        now = datetime.now()
        current_year = now.year
        current_month = now.month

        if current_month < 7:
            # Current is H1 (incomplete), so Last Complete is Previous Year H2
            expected_year = current_year - 1
            expected_half = 2
        else:
            # Current is H2 (incomplete), so Last Complete is Current Year H1
            expected_year = current_year
            expected_half = 1

        if expected_half == 1:
            assert start == date(expected_year, 1, 1)
            assert end == date(expected_year, 6, 30)
        else:
            assert start == date(expected_year, 7, 1)
            assert end == date(expected_year, 12, 31)

    def test_get_period_gains_half_takes_precedence_over_dates(self):
        """For GAINS, half-year takes precedence over date range."""
        params = ReportParams.model_validate(
            {
                "type": "gains",
                "half": "2023-1",
                "start": "2025-01-15",
                "end": "2025-02-20",
            },
        )
        start, end = params.get_period()
        # Should use half-year, not dates
        assert start == date(2023, 1, 1)
        assert end == date(2023, 6, 30)

    def test_get_period_income_with_half_h1(self):
        """get_period for INCOME with half-year H1 returns Jan-Jun."""
        params = ReportParams.model_validate({"type": "income", "half": "2023-1"})
        start, end = params.get_period()
        assert start == date(2023, 1, 1)
        assert end == date(2023, 6, 30)

    def test_get_period_income_with_half_h2(self):
        """get_period for INCOME with half-year H2 returns Jul-Dec."""
        params = ReportParams.model_validate({"type": "income", "half": "2023-2"})
        start, end = params.get_period()
        assert start == date(2023, 7, 1)
        assert end == date(2023, 12, 31)

    def test_get_period_income_with_dates(self):
        """get_period for INCOME with date range returns those dates."""
        params = ReportParams.model_validate(
            {"type": "income", "start": "2025-01-15", "end": "2025-02-20"},
        )
        start, end = params.get_period()
        assert start == date(2025, 1, 15)
        assert end == date(2025, 2, 20)

    def test_get_period_income_default_current_month(self):
        """get_period for INCOME defaults to current month (from 1st to today)."""
        params = ReportParams.model_validate({"type": "income"})
        start, end = params.get_period()

        now = datetime.now()
        expected_start = date(now.year, now.month, 1)
        expected_end = now.date()

        assert start == expected_start
        assert end == expected_end

    def test_parse_half_dash_format(self):
        """_parse_half should parse dash format correctly."""
        params = ReportParams.model_validate({"half": "2023-2"})
        result = params._parse_half()
        assert result == (2023, 2)

    def test_parse_half_compact_format(self):
        """_parse_half should parse compact format correctly."""
        params = ReportParams.model_validate({"half": "20232"})
        result = params._parse_half()
        assert result == (2023, 2)

    def test_parse_half_none(self):
        """_parse_half should return None when half is not provided."""
        params = ReportParams.model_validate({})
        result = params._parse_half()
        assert result is None

    def test_parse_dates(self):
        """_parse_dates should parse date strings correctly."""
        params = ReportParams.model_validate(
            {"start": "2025-01-15", "end": "2025-02-20"},
        )
        start_date_obj, end_date_obj = params._parse_dates()
        assert start_date_obj == date(2025, 1, 15)
        assert end_date_obj == date(2025, 2, 20)

    def test_parse_dates_none(self):
        """_parse_dates should return None when dates are not provided."""
        params = ReportParams.model_validate({})
        start_date_obj, end_date_obj = params._parse_dates()
        assert start_date_obj is None
        assert end_date_obj is None

    def test_parse_dates_partial(self):
        """_parse_dates should handle partial dates."""
        params = ReportParams.model_validate({"start": "2025-01-15"})
        # Note: validate_date_range sets end_date = start_date when only start is provided
        start_date_obj, end_date_obj = params._parse_dates()
        assert start_date_obj == date(2025, 1, 15)
        assert end_date_obj == date(2025, 1, 15)  # Auto-set by validator

    def test_alias_start_to_start_date(self):
        """Field alias 'start' should map to start_date."""
        params = ReportParams.model_validate({"start": "2025-01-15"})
        assert params.start_date == "2025-01-15"

    def test_alias_end_to_end_date(self):
        """Field alias 'end' should map to end_date."""
        params = ReportParams.model_validate({"end": "2025-01-15"})
        assert params.end_date == "2025-01-15"
