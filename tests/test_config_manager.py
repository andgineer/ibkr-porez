import json
import pytest
from unittest.mock import patch
import allure
from ibkr_porez.config import ConfigManager
from ibkr_porez.models import UserConfig


@allure.epic("Configuration")
@allure.feature("ConfigManager")
class TestConfigManager:
    @pytest.fixture
    def mock_config_dir(self, tmp_path):
        """Mock platformdirs to use tmp_path"""
        with patch("ibkr_porez.config.user_config_dir", return_value=str(tmp_path)):
            yield tmp_path

    def test_config_path(self, mock_config_dir):
        """Verify config_path property returns expected path."""
        cm = ConfigManager()
        expected = mock_config_dir / "config.json"

        assert cm.config_path == expected
        # Ensure dir was created
        assert mock_config_dir.exists()

    def test_save_load_roundtrip(self, mock_config_dir):
        """Verify save_config and load_config work together."""
        cm = ConfigManager()

        # 1. Create Config Object
        cfg = UserConfig(
            ibkr_token="test_token",
            ibkr_query_id="12345",
            personal_id="9999999999999",
            full_name="Unit Tester",
            address="Test St 1",
            city_code="111",
        )

        # 2. Save
        cm.save_config(cfg)

        # Check file exists and has content
        assert cm.config_path.exists()
        with open(cm.config_path) as f:
            data = json.load(f)
            assert data["ibkr_token"] == "test_token"

        # 3. Load
        loaded_cfg = cm.load_config()
        assert loaded_cfg.ibkr_token == "test_token"
        assert loaded_cfg.full_name == "Unit Tester"
        assert loaded_cfg.city_code == "111"

    def test_load_missing_file(self, mock_config_dir):
        """Verify load_config returns empty/default config if file missing."""
        cm = ConfigManager()
        # Ensure no file
        if cm.config_path.exists():
            cm.config_path.unlink()

        cfg = cm.load_config()
        assert cfg.ibkr_token == ""
        assert cfg.full_name == ""

    def test_load_corrupted_file(self, mock_config_dir):
        """Verify load_config returns default config if file corrupted."""
        cm = ConfigManager()

        # Write garbage
        with open(cm.config_path, "w") as f:
            f.write("{ invalid json")

        cfg = cm.load_config()
        # Should handle JSONDecodeError and return empty config
        assert cfg.ibkr_token == ""
        assert isinstance(cfg, UserConfig)
