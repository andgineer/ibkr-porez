import os
import pytest
import tempfile
import shutil
from pathlib import Path
from unittest.mock import patch
from ibkr_porez.models import UserConfig

TEST_ROOT = Path(__file__).resolve().parent.parent
TEST_HOME_DIR = TEST_ROOT / ".tmp-home" / "home"
TEST_LOG_DIR = TEST_ROOT / ".tmp-home" / "pytest-logs"
TEST_HOME_DIR.mkdir(parents=True, exist_ok=True)
TEST_LOG_DIR.mkdir(parents=True, exist_ok=True)

# Force all filesystem-sensitive defaults into test sandbox directories.
os.environ["HOME"] = str(TEST_HOME_DIR)
os.environ["USERPROFILE"] = str(TEST_HOME_DIR)
os.environ["XDG_CONFIG_HOME"] = str(TEST_HOME_DIR / ".config")
os.environ["XDG_DATA_HOME"] = str(TEST_HOME_DIR / ".local" / "share")
os.environ["APPDATA"] = str(TEST_HOME_DIR / "AppData" / "Roaming")
os.environ["LOCALAPPDATA"] = str(TEST_HOME_DIR / "AppData" / "Local")
os.environ.setdefault("IBKR_POREZ_LOG_DIR", str(TEST_LOG_DIR))
os.environ["QT_QPA_PLATFORM"] = "offscreen"


@pytest.fixture(scope="function", autouse=True)
def mock_config_for_all_tests(request):
    """
    Automatically mock config_manager.load_config() and user_data_dir for all tests.
    This prevents tests from using user's real config and creating files in real data folders.

    Can be disabled for specific tests by marking them with @pytest.mark.no_config_mock
    """
    if "no_config_mock" in request.keywords:
        yield
        return

    # Create a temporary directory for this test
    temp_dir = tempfile.mkdtemp(prefix="ibkr-porez-test-")
    temp_path = Path(temp_dir)

    try:
        mock_config = UserConfig(
            full_name="Test User", address="Test Address", data_dir=None, output_folder=None
        )
        with patch("ibkr_porez.storage.config_manager.load_config", return_value=mock_config):
            with patch(
                "ibkr_porez.declaration_manager.get_effective_output_dir_path",
                return_value=temp_path,
            ):
                with patch("ibkr_porez.storage.user_data_dir", lambda app: str(temp_path)):
                    yield
    finally:
        # Cleanup: remove the temporary directory
        if temp_path.exists():
            shutil.rmtree(temp_path, ignore_errors=True)


@pytest.fixture
def resources_path():
    """Return the path to the tests/resources directory."""
    return Path(__file__).parent / "resources"
