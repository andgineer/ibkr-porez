import pytest
from pathlib import Path


@pytest.fixture
def resources_path():
    """Return the path to the tests/resources directory."""
    return Path(__file__).parent / "resources"
