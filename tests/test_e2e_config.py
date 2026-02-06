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
        Scenario: User runs `config` command and provides all inputs.
        Expect: Config file created with correct values.
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
        ]

        # Join inputs with newlines for Click Prompt
        input_str = "\n".join(inputs)

        result = runner.invoke(ibkr_porez, ["config"], input=input_str)

        assert result.exit_code == 0
        assert "Configuration saved successfully" in result.output

        # Verify File
        config_path = mock_config_dir / "config.json"
        assert config_path.exists()

        with open(config_path) as f:
            data = json.load(f)

        assert data["ibkr_token"] == "my_token"
        assert data["ibkr_query_id"] == "my_query"
        assert data["personal_id"] == "1234567890123"
        assert data["city_code"] == "223"

    def test_config_update(self, runner, mock_config_dir):
        """
        Scenario: User updates existing config (pressing Enter to keep defaults).
        """
        # 1. Create initial config
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

        # 2. Run config, change token, keep others default
        inputs = [
            "new_token",  # Change Token
            "",  # Keep Query ID
            "",  # Keep JMBG
            "",  # Keep Name
            "",  # Keep Address
            "",  # Keep City
            "",  # Keep Phone
            "",  # Keep Email
        ]

        result = runner.invoke(ibkr_porez, ["config"], input="\n".join(inputs))

        assert result.exit_code == 0

        # Verify
        with open(mock_config_dir / "config.json") as f:
            data = json.load(f)

        assert data["ibkr_token"] == "new_token"
        assert data["ibkr_query_id"] == "old_query"  # Kept default
