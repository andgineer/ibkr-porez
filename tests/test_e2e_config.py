import allure
import json
import pytest
from unittest.mock import patch

from click.testing import CliRunner

from ibkr_porez.config import config_manager
from ibkr_porez.main import ibkr_porez


pytestmark = pytest.mark.no_config_mock


@pytest.fixture
def mock_config_dir(tmp_path):
    # Patch where ConfigManager looks for config dir
    with patch("ibkr_porez.config.user_config_dir", lambda app: str(tmp_path)):
        # We need to re-instantiate ConfigManager or ensure it uses the patched path.
        # ConfigManager is instantiated as `config_manager` global in `config.py`.
        # So patching the class attribute or property might be safer,
        # BUT `config_manager` is already created.
        # Actually `config_manager` calculates path in `__init__`.
        # So we must patch the instance's `_config_dir` OR patch `user_config_dir` BEFORE import (too late usually).
        # Better strategy: Patch `ibkr_porez.main.config_manager._config_dir` and `_config_file`.

        # However, `config` command uses `config_manager` imported from `config`.
        # Let's patch the instance methods/properties if possible, or create a fresh one?
        # A simpler way is to patch `ibkr_porez.config.config_manager._config_dir`.

        original_dir = config_manager._config_dir
        original_file = config_manager._config_file

        config_manager._config_dir = tmp_path
        config_manager._config_file = tmp_path / "config.json"
        config_manager._ensure_config_dir()

        yield tmp_path

        # Restore (though pytests run in sequence, good practice)
        config_manager._config_dir = original_dir
        config_manager._config_file = original_file


@pytest.fixture
def runner():
    return CliRunner()


@allure.epic("End-to-end")
@allure.feature("config")
class TestE2EConfig:
    def test_config_setup(self, runner, mock_config_dir):
        """
        Scenario: User runs `config` command for first time (empty config).
        Expect: Config file created with correct values, all fields requested automatically.
        """
        inputs = [
            "my_token",  # Token
            "my_query",  # Query ID
            "1234567890123",  # JMBG
            "Andrei Sorokin",  # Name
            "Test Str 1",  # Address
            "223",  # City Code
            "060123456",  # Phone
            "test@example.com",  # Email
            "",  # Data Directory (default)
            "",  # Output Folder (default)
        ]

        input_str = "\n".join(inputs) + "\n"

        result = runner.invoke(ibkr_porez, ["config"], input=input_str)

        assert result.exit_code == 0
        assert "Initial Configuration Setup" in result.output
        assert "Configuration saved successfully" in result.output

        config_path = mock_config_dir / "config.json"
        assert config_path.exists()

        with open(config_path) as f:
            data = json.load(f)

        assert data["ibkr_token"] == "my_token"
        assert data["ibkr_query_id"] == "my_query"
        assert data["personal_id"] == "1234567890123"
        assert data["full_name"] == "Andrei Sorokin"
        assert data["address"] == "Test Str 1"
        assert data["city_code"] == "223"
        assert data["phone"] == "060123456"
        assert data["email"] == "test@example.com"

    def test_config_update(self, runner, mock_config_dir):
        """
        Scenario: User updates existing config by selecting specific fields.
        """
        initial_data = {
            "ibkr_token": "old_token",
            "ibkr_query_id": "old_query",
            "personal_id": "111",
            "full_name": "Old Name",
            "address": "Old Addr",
            "city_code": "000",
            "phone": "000",
            "email": "old@email.com",
        }
        with open(mock_config_dir / "config.json", "w") as f:
            json.dump(initial_data, f)

        inputs = [
            "1",  # Select field 1 (IBKR Flex Token)
            "new_token",  # New token value
        ]

        result = runner.invoke(ibkr_porez, ["config"], input="\n".join(inputs) + "\n")

        assert result.exit_code == 0
        assert "Current Configuration" in result.output
        assert "Configuration saved successfully" in result.output

        with open(mock_config_dir / "config.json") as f:
            data = json.load(f)

        assert data["ibkr_token"] == "new_token"
        assert data["ibkr_query_id"] == "old_query"
        assert data["full_name"] == "Old Name"

    def test_config_update_multiple_fields(self, runner, mock_config_dir):
        """
        Scenario: User updates multiple fields at once.
        """
        initial_data = {
            "ibkr_token": "old_token",
            "ibkr_query_id": "old_query",
            "personal_id": "111",
            "full_name": "Old Name",
            "address": "Old Addr",
            "city_code": "000",
            "phone": "000",
            "email": "old@email.com",
        }
        with open(mock_config_dir / "config.json", "w") as f:
            json.dump(initial_data, f)

        inputs = [
            "1,4",  # Select fields 1 (Token) and 4 (Full Name)
            "new_token",  # New token
            "New Name",  # New name
        ]

        result = runner.invoke(ibkr_porez, ["config"], input="\n".join(inputs) + "\n")

        assert result.exit_code == 0

        with open(mock_config_dir / "config.json") as f:
            data = json.load(f)

        assert data["ibkr_token"] == "new_token"
        assert data["full_name"] == "New Name"
        assert data["ibkr_query_id"] == "old_query"

    def test_config_update_all_fields(self, runner, mock_config_dir):
        """
        Scenario: User selects 'all' to update all fields.
        """
        initial_data = {
            "ibkr_token": "old_token",
            "ibkr_query_id": "old_query",
            "personal_id": "111",
            "full_name": "Old Name",
            "address": "Old Addr",
            "city_code": "000",
            "phone": "000",
            "email": "old@email.com",
        }
        with open(mock_config_dir / "config.json", "w") as f:
            json.dump(initial_data, f)

        inputs = [
            "all",  # Select all fields
            "new_token",
            "new_query",
            "222",
            "New Name",
            "New Addr",
            "223",
            "111",
            "new@email.com",
            "",  # Data dir (default)
            "",  # Output folder (default)
        ]

        result = runner.invoke(ibkr_porez, ["config"], input="\n".join(inputs) + "\n")

        assert result.exit_code == 0

        with open(mock_config_dir / "config.json") as f:
            data = json.load(f)

        assert data["ibkr_token"] == "new_token"
        assert data["ibkr_query_id"] == "new_query"
        assert data["personal_id"] == "222"
        assert data["full_name"] == "New Name"
        assert data["address"] == "New Addr"
        assert data["city_code"] == "223"

    def test_config_skip_update(self, runner, mock_config_dir):
        """
        Scenario: User presses Enter to skip updating any fields.
        """
        initial_data = {
            "ibkr_token": "old_token",
            "ibkr_query_id": "old_query",
            "personal_id": "111",
            "full_name": "Old Name",
            "address": "Old Addr",
            "city_code": "000",
            "phone": "000",
            "email": "old@email.com",
        }
        with open(mock_config_dir / "config.json", "w") as f:
            json.dump(initial_data, f)

        result = runner.invoke(ibkr_porez, ["config"], input="\n")

        assert result.exit_code == 0
        assert "No fields selected. Configuration unchanged." in result.output

        with open(mock_config_dir / "config.json") as f:
            data = json.load(f)

        assert data["ibkr_token"] == "old_token"
        assert data["full_name"] == "Old Name"

    def test_config_invalid_selection(self, runner, mock_config_dir):
        """
        Scenario: User enters invalid field selection.
        """
        initial_data = {
            "ibkr_token": "old_token",
            "ibkr_query_id": "old_query",
            "personal_id": "111",
            "full_name": "Old Name",
            "address": "Old Addr",
            "city_code": "000",
            "phone": "000",
            "email": "old@email.com",
        }
        with open(mock_config_dir / "config.json", "w") as f:
            json.dump(initial_data, f)

        result = runner.invoke(ibkr_porez, ["config"], input="invalid\n")

        assert result.exit_code == 0
        assert "Invalid selection" in result.output

    def test_config_data_dir_custom(self, runner, mock_config_dir):
        """
        Scenario: User sets custom data directory.
        """
        inputs = [
            "token",
            "query",
            "123",
            "Name",
            "Addr",
            "223",
            "123",
            "email@test.com",
            str(mock_config_dir / "custom_data"),  # Custom data dir
            "",  # Output folder (default)
        ]

        result = runner.invoke(ibkr_porez, ["config"], input="\n".join(inputs) + "\n")

        assert result.exit_code == 0

        with open(mock_config_dir / "config.json") as f:
            data = json.load(f)

        assert data["data_dir"] == str(mock_config_dir / "custom_data")
