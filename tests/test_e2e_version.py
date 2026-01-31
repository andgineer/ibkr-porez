import allure
from ibkr_porez import __version__
from ibkr_porez.main import ibkr_porez
from click.testing import CliRunner


@allure.epic("End-to-end")
@allure.feature("version")
def test_version():
    assert __version__


@allure.epic("End-to-end")
@allure.feature("version")
def test_version_option():
    runner = CliRunner()
    result = runner.invoke(ibkr_porez, ["--version"])
    assert result.exit_code == 0
    assert __version__ in result.output
