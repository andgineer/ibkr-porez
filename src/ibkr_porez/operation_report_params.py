"""Parameter validation for report commands."""

import re
from datetime import date, datetime
from enum import Enum

from pydantic import BaseModel, Field, field_validator, model_validator


class ReportType(str, Enum):
    """Report type enumeration."""

    GAINS = "gains"
    INCOME = "income"


class ReportParams(BaseModel):
    """Validated parameters for report generation."""

    type: ReportType = ReportType.GAINS
    half: str | None = None
    start_date: str | None = Field(None, alias="start")
    end_date: str | None = Field(None, alias="end")
    force: bool = False

    @field_validator("start_date", "end_date")
    @classmethod
    def validate_date_format(cls, v: str | None) -> str | None:
        """Validate date string format."""
        if v is None:
            return None
        try:
            datetime.strptime(v, "%Y-%m-%d")
            return v
        except ValueError as e:
            raise ValueError(
                f"Invalid date format: {v}. Use YYYY-MM-DD (e.g. 2025-01-15)",
            ) from e

    @field_validator("half")
    @classmethod
    def validate_half_format(cls, v: str | None) -> str | None:
        """Validate half-year string format."""
        if v is None:
            return None

        # Formats: 2023-2, 20232
        m_dash = re.match(r"^(\d{4})-(\d)$", v)
        m_compact = re.match(r"^(\d{4})(\d)$", v)

        if m_dash:
            target_half = int(m_dash.group(2))
        elif m_compact:
            target_half = int(m_compact.group(2))
        else:
            raise ValueError(
                f"Invalid format: {v}. Use YYYY-H (e.g. 2023-2) or YYYYH (e.g. 20232)",
            )

        if target_half not in [1, 2]:
            raise ValueError("Half-year must be 1 or 2.")

        return v

    def _parse_half(self) -> tuple[int, int] | None:
        """Parse half-year string to (year, half)."""
        if self.half is None:
            return None

        # Formats: 2023-2, 20232
        m_dash = re.match(r"^(\d{4})-(\d)$", self.half)
        m_compact = re.match(r"^(\d{4})(\d)$", self.half)

        if m_dash:
            target_year = int(m_dash.group(1))
            target_half = int(m_dash.group(2))
        elif m_compact:
            target_year = int(m_compact.group(1))
            target_half = int(m_compact.group(2))
        else:
            return None  # Should not happen after validation

        return (target_year, target_half)

    def _parse_dates(self) -> tuple[date | None, date | None]:
        """Parse date strings to date objects."""
        start_date_obj = None
        end_date_obj = None

        if self.start_date:
            start_date_obj = datetime.strptime(self.start_date, "%Y-%m-%d").date()
        if self.end_date:
            end_date_obj = datetime.strptime(self.end_date, "%Y-%m-%d").date()

        return start_date_obj, end_date_obj

    @model_validator(mode="after")
    def validate_date_range(self) -> "ReportParams":
        """Validate that start date is before or equal to end date."""
        # If --start is filled and --end is empty, set --end equal to --start
        if self.start_date and not self.end_date:
            self.end_date = self.start_date

        start_date_obj, end_date_obj = self._parse_dates()

        if start_date_obj and end_date_obj and start_date_obj > end_date_obj:
            raise ValueError("Start date must be before or equal to end date.")

        return self

    def get_period(self) -> tuple[date, date]:  # noqa: C901,PLR0911,PLR0912
        """
        Determine report period based on parameters.

        Returns:
            tuple[date, date]: (start_date, end_date)

        Raises:
            ValueError: If period cannot be determined.
        """
        start_date_obj, end_date_obj = self._parse_dates()
        half_parsed = self._parse_half()
        target_year: int | None = None
        target_half: int | None = None

        # Parse half if provided
        if half_parsed:
            target_year, target_half = half_parsed

        # Determine period based on type
        if self.type == ReportType.GAINS:
            # For gains: half takes precedence, then dates, then default (last full half-year)
            if half_parsed and target_year and target_half:
                if target_half == 1:
                    return date(target_year, 1, 1), date(target_year, 6, 30)
                return date(target_year, 7, 1), date(target_year, 12, 31)

            if start_date_obj and end_date_obj:
                return start_date_obj, end_date_obj

            # If --start or --end is provided (but not both), handle special cases
            # If only --start is provided, --end defaults to --start
            # (handled in validate_date_range)
            if start_date_obj and not end_date_obj:
                # This should not happen due to validate_date_range, but handle it
                return start_date_obj, start_date_obj
            if end_date_obj and not start_date_obj:
                # User provided --end but not --start, use start of current month
                now = datetime.now()
                start_of_month = date(now.year, now.month, 1)
                return start_of_month, end_date_obj

            # Default: Last COMPLETE half-year (when nothing is provided)
            # This is the default behavior for gains when no parameters are specified
            now = datetime.now()
            current_year = now.year
            current_month = now.month

            if current_month < 7:  # noqa: PLR2004
                # Current is H1 (incomplete), so Last Complete is Previous Year H2
                target_year = current_year - 1
                target_half = 2
            else:
                # Current is H2 (incomplete), so Last Complete is Current Year H1
                target_year = current_year
                target_half = 1

            if target_half == 1:
                return date(target_year, 1, 1), date(target_year, 6, 30)
            return date(target_year, 7, 1), date(target_year, 12, 31)

        if self.type == ReportType.INCOME:
            # For income: can accept half, dates, or use default (current month)
            if half_parsed and target_year and target_half:
                if target_half == 1:
                    return date(target_year, 1, 1), date(target_year, 6, 30)
                return date(target_year, 7, 1), date(target_year, 12, 31)

            if start_date_obj and end_date_obj:
                return start_date_obj, end_date_obj

            # Default: current month (from 1st to today)
            now = datetime.now()
            start_of_month = date(now.year, now.month, 1)
            today = now.date()
            return start_of_month, today

        raise ValueError(f"Unknown report type: {self.type}")
