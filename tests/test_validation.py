"""Tests for validation error handling utilities."""

import allure
from unittest.mock import MagicMock
from pydantic import ValidationError, BaseModel, field_validator

from ibkr_porez.validation import format_validation_error, handle_validation_error


class SampleModel(BaseModel):
    """Sample model for validation errors."""

    field1: str
    field2: int

    @field_validator("field2")
    @classmethod
    def validate_field2(cls, v: int) -> int:
        """Validate field2."""
        if v < 0:
            raise ValueError("field2 must be non-negative")
        return v


@allure.epic("Tax")
@allure.feature("PPDG-3R (gains)")
class TestValidationErrorHandling:
    """Test validation error formatting and handling."""

    def test_format_validation_error_single_error(self):
        """format_validation_error should extract single error message."""
        try:
            SampleModel.model_validate({"field1": "test", "field2": -1})
        except ValidationError as e:
            result = format_validation_error(e)
            assert "field2 must be non-negative" in result
            assert "For further information visit" not in result

    def test_format_validation_error_removes_pydantic_link(self):
        """format_validation_error should remove Pydantic documentation links."""
        # Create a ValidationError by trying to validate with missing required field
        try:
            SampleModel.model_validate({})
        except ValidationError as e:
            # Manually add Pydantic link to error message to test removal
            error_dict = e.errors()[0]
            error_dict["msg"] = (
                f"{error_dict['msg']}. For further information visit https://errors.pydantic.dev"
            )
            result = format_validation_error(e)
            assert "field required" in result.lower() or "field1" in result.lower()
            assert "For further information visit" not in result
            assert "https://errors.pydantic.dev" not in result

    def test_format_validation_error_multiple_errors(self):
        """format_validation_error should combine multiple error messages."""
        try:
            SampleModel.model_validate({})
        except ValidationError as e:
            result = format_validation_error(e)
            # Should contain both error messages
            assert "field1" in result.lower() or "field required" in result.lower()
            assert "field2" in result.lower() or "field required" in result.lower()

    def test_format_validation_error_empty_message(self):
        """format_validation_error should handle empty error messages."""
        # Create error with empty message by modifying a real error
        try:
            SampleModel.model_validate({"field1": "test"})
        except ValidationError as e:
            # Modify error message to be empty
            error_dict = e.errors()[0]
            original_msg = error_dict["msg"]
            error_dict["msg"] = ""
            result = format_validation_error(e)
            # Should return something (fallback to string representation)
            assert isinstance(result, str)
            # Restore for cleanup
            error_dict["msg"] = original_msg

    def test_format_validation_error_fallback_to_string_representation(self):
        """format_validation_error should fallback to string representation if no messages."""
        # Create error and clear all messages to test fallback
        try:
            SampleModel.model_validate({"field1": "test"})
        except ValidationError as e:
            # Clear all messages
            for error_dict in e.errors():
                error_dict["msg"] = ""
            result = format_validation_error(e)
            # Should use fallback logic
            assert isinstance(result, str)

    def test_format_validation_error_extracts_value_error(self):
        """format_validation_error should extract message from 'Value error,' prefix."""
        # Pydantic automatically adds "Value error," prefix, so test with real error
        try:
            SampleModel.model_validate({"field1": "test", "field2": -1})
        except ValidationError as e:
            result = format_validation_error(e)
            # Should contain the actual error message
            assert "field2 must be non-negative" in result
            # The function should handle "Value error," prefix if present in string representation
            # but Pydantic v2 puts it in the msg field, so format_validation_error extracts from errors()

    def test_format_validation_error_handles_custom_error_message(self):
        """format_validation_error should handle custom error messages."""
        try:
            SampleModel.model_validate({"field1": "test", "field2": -5})
        except ValidationError as e:
            result = format_validation_error(e)
            assert "field2 must be non-negative" in result

    def test_handle_validation_error_calls_console_print(self):
        """handle_validation_error should call console.print with formatted message."""
        mock_console = MagicMock()
        try:
            SampleModel.model_validate({"field1": "test", "field2": -1})
        except ValidationError as e:
            handle_validation_error(e, mock_console)

            # Verify console.print was called
            mock_console.print.assert_called_once()
            call_args = mock_console.print.call_args[0][0]
            assert "[red]" in call_args
            assert "field2 must be non-negative" in call_args
            assert "[/red]" in call_args

    def test_handle_validation_error_removes_pydantic_link(self):
        """handle_validation_error should remove Pydantic links before printing."""
        mock_console = MagicMock()
        # Use ReportParams which can have Pydantic links in error messages
        from ibkr_porez.report_params import ReportParams

        try:
            ReportParams.model_validate({"half": "2023-3"})
        except ValidationError as e:
            handle_validation_error(e, mock_console)

            call_args = mock_console.print.call_args[0][0]
            # Should contain the error message
            assert "Half-year must be 1 or 2" in call_args
            # Should not contain Pydantic documentation link
            assert "For further information visit" not in call_args
            assert "https://errors.pydantic.dev" not in call_args

    def test_handle_validation_error_with_multiple_errors(self):
        """handle_validation_error should handle multiple validation errors."""
        mock_console = MagicMock()
        try:
            SampleModel.model_validate({})
        except ValidationError as e:
            handle_validation_error(e, mock_console)

            # Verify console.print was called
            mock_console.print.assert_called_once()
            call_args = mock_console.print.call_args[0][0]
            assert "[red]" in call_args
            assert "[/red]" in call_args

    def test_format_validation_error_with_real_validation_error(self):
        """format_validation_error should work with real Pydantic ValidationError."""
        from ibkr_porez.report_params import ReportParams

        try:
            ReportParams.model_validate({"half": "2023-3"})
        except ValidationError as e:
            result = format_validation_error(e)
            assert "Half-year must be 1 or 2" in result
            assert "For further information visit" not in result

    def test_format_validation_error_with_date_format_error(self):
        """format_validation_error should format date format errors correctly."""
        from ibkr_porez.report_params import ReportParams

        try:
            ReportParams.model_validate({"from": "2025-01-15-extra"})
        except ValidationError as e:
            result = format_validation_error(e)
            assert "Invalid date format" in result
            assert "2025-01-15-extra" in result
            assert "For further information visit" not in result

    def test_handle_validation_error_integration(self):
        """Integration test: handle_validation_error with real console."""
        from rich.console import Console
        from io import StringIO

        console = Console(file=StringIO(), force_terminal=False)
        try:
            SampleModel.model_validate({"field1": "test", "field2": -1})
        except ValidationError as e:
            handle_validation_error(e, console)

            # Verify output contains error message
            output = console.file.getvalue()
            assert "field2 must be non-negative" in output
