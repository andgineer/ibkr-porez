import allure
import pytest
from unittest.mock import patch, MagicMock
from click.testing import CliRunner
from ibkr_porez.main import ibkr_porez
from ibkr_porez.storage import Storage


@pytest.fixture
def mock_user_data_dir(tmp_path):
    with patch("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path)):
        # Ensure dirs exist
        s = Storage()
        s._ensure_dirs()
        yield tmp_path


@pytest.fixture
def runner():
    return CliRunner()


@allure.epic("End-to-end")
@allure.feature("Fetching from IBKR")
class TestE2EFetching:
    @patch("ibkr_porez.operation_get.NBSClient")
    @patch("ibkr_porez.ibkr_flex_query.IBKRClient.fetch_latest_report")
    @patch("ibkr_porez.main.config_manager")
    def test_get_command_complex_fetch(
        self,
        mock_cfg_mgr,
        mock_fetch,
        mock_nbs_cls_get,
        runner,
        mock_user_data_dir,
        resources_path,
    ):
        """
        Scenario: Fetch complex Flex Query with multiple trades, splits, and cash interactions.
        Expect: All transactions parsed and stored correctly.
        """
        # Mock ConfigManager.load_config()
        mock_cfg_mgr.load_config.return_value = MagicMock(
            ibkr_token="test_token", ibkr_query_id="test_query"
        )

        # Mock NBS (in operation_get)
        mock_nbs_get = mock_nbs_cls_get.return_value
        mock_nbs_get.get_rate.return_value = None

        # Load complex XML
        with open(resources_path / "complex_flex.xml", "rb") as f:
            mock_fetch.return_value = f.read()

        result = runner.invoke(ibkr_porez, ["get"])

        assert result.exit_code == 0

        # Verify Storage
        s = Storage()
        txs = s.get_transactions()

        # Expected Counts:
        # Trades: AAPL(1), TSLA(1), GOOG(2 split) = 4
        # Cash: Div KO(1), Tax KO(1), Interest(1) = 3
        # Total: 7
        assert len(txs) == 7
        assert "Fetched 7 transactions" in result.output

        # Verify Specifics
        aapl = txs[txs["symbol"] == "AAPL"].iloc[0]
        assert aapl["quantity"] == 10.0
        assert aapl["transaction_id"] == "XML_AAPL_BUY_1"

        tsla = txs[txs["symbol"] == "TSLA"].iloc[0]
        assert tsla["quantity"] == -5.0
        assert tsla["transaction_id"] == "XML_TSLA_SELL_1"

        goog = txs[txs["symbol"] == "GOOG"]
        assert len(goog) == 2
        assert "XML_GOOG_SPLIT_1" in goog["transaction_id"].values

        div = txs[(txs["symbol"] == "KO") & (txs["type"] == "DIVIDEND")].iloc[0]
        assert div["amount"] == 50.0

        tax = txs[(txs["symbol"] == "KO") & (txs["type"] == "WITHHOLDING_TAX")].iloc[0]
        assert abs(tax["amount"]) == 7.5

    @patch("ibkr_porez.operation_import.NBSClient")
    @patch("ibkr_porez.main.config_manager")
    def test_import_command_complex(
        self, mock_cfg_mgr, mock_nbs_cls, runner, mock_user_data_dir, resources_path
    ):
        # Import uses operation_import, so patch NBSClient there
        """
        Scenario: Import complex CSV.
        Expect: All transactions parsed.
        """
        mock_cfg_mgr.load_config.return_value = MagicMock()
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = None

        csv_path = resources_path / "complex_activity.csv"

        result = runner.invoke(ibkr_porez, ["import", str(csv_path)])

        assert result.exit_code == 0

        # Expected:
        # Trades: AAPL, MSFT = 2
        # Cash: Div KO, Tax KO = 2
        # Total: 4
        assert "Parsed 4 transactions" in result.output

        s = Storage()
        txs = s.get_transactions()
        assert len(txs) == 4

        msft = txs[txs["symbol"] == "MSFT"].iloc[0]
        assert msft["transaction_id"] == "csv-MSFT_ONLY_CSV"

    @patch("ibkr_porez.operation_get.NBSClient")
    @patch("ibkr_porez.operation_import.NBSClient")
    @patch("ibkr_porez.ibkr_flex_query.IBKRClient.fetch_latest_report")
    @patch("ibkr_porez.main.config_manager")
    def test_workflow_partial_upgrade(
        self,
        mock_cfg_mgr,
        mock_fetch,
        mock_nbs_cls_import,
        mock_nbs_cls_get,
        runner,
        mock_user_data_dir,
        resources_path,
    ):
        """
        Scenario (Partial Upgrade):
        1. Import CSV (contains AAPL, MSFT).
        2. Fetch Flex Query (contains AAPL, TSLA, GOOG). MSFT is missing in XML (different date/filter).

        Expect:
        - AAPL (CSV) -> Replaced by AAPL (XML).
        - MSFT (CSV) -> RETAINED (because XML dates [Jan, Feb] don't cover MSFT date [Apr]).
        - TSLA/GOOG (XML) -> Added.
        """
        mock_cfg_mgr.load_config.return_value = MagicMock(ibkr_token="t", ibkr_query_id="q")

        mock_nbs_import = mock_nbs_cls_import.return_value
        mock_nbs_import.get_rate.return_value = None
        mock_nbs_get = mock_nbs_cls_get.return_value
        mock_nbs_get.get_rate.return_value = None

        with open(resources_path / "complex_flex.xml", "rb") as f:
            mock_fetch.return_value = f.read()

        # 1. Import CSV
        runner.invoke(ibkr_porez, ["import", str(resources_path / "complex_activity.csv")])

        s = Storage()
        assert len(s.get_transactions()) == 4
        assert "csv-XML_AAPL_BUY_1" in s.get_transactions()["transaction_id"].values

        # 2. Fetch XML
        result = runner.invoke(ibkr_porez, ["get"])
        assert result.exit_code == 0

        # Logic Check:
        # XML IDs: XML_AAPL_BUY_1, XML_TSLA, XML_GOOG_1, XML_GOOG_2, XML_DIV, XML_TAX, XML_INT
        # CSV IDs in DB: csv-XML_AAPL_BUY_1 (Jan 1), csv-MSFT (Apr 1), csv-Div, csv-Tax (Mar 15).

        # XML Dates: Jan 1, Jan 5, Feb 1, Mar 15, Mar 31.
        # CSV Dates: Jan 1 (AAPL), Apr 1 (MSFT), Mar 15 (Div/Tax).

        # Result:
        # Jan 1: XML covers -> AAPL replaced.
        # Mar 15: XML covers -> Div/Tax replaced.
        # Apr 1: XML does NOT cover -> MSFT retained.

        # Total expected:
        # XML items (7) + Retained CSV MSFT (1) = 8.

        txs = s.get_transactions()
        assert len(txs) == 8

        # Verify AAPL is XML
        aapl = txs[txs["symbol"] == "AAPL"].iloc[0]
        assert aapl["transaction_id"] == "XML_AAPL_BUY_1"

        # Verify MSFT is CSV
        msft = txs[txs["symbol"] == "MSFT"].iloc[0]
        assert msft["transaction_id"] == "csv-MSFT_ONLY_CSV"

    @patch("ibkr_porez.operation_get.NBSClient")
    @patch("ibkr_porez.operation_import.NBSClient")
    @patch("ibkr_porez.ibkr_flex_query.IBKRClient.fetch_latest_report")
    @patch("ibkr_porez.main.config_manager")
    def test_workflow_skip_duplicates(
        self,
        mock_cfg_mgr,
        mock_fetch,
        mock_nbs_cls_import,
        mock_nbs_cls_get,
        runner,
        mock_user_data_dir,
        resources_path,
    ):
        """
        Scenario:
        1. Fetch Flex Query.
        2. Import CSV.

        Expect:
        - Common items (AAPL, Divs) skipped.
        - New items in CSV (MSFT) added.
        """
        mock_cfg_mgr.load_config.return_value = MagicMock(ibkr_token="t", ibkr_query_id="q")

        mock_nbs_import = mock_nbs_cls_import.return_value
        mock_nbs_import.get_rate.return_value = None
        mock_nbs_get = mock_nbs_cls_get.return_value
        mock_nbs_get.get_rate.return_value = None

        with open(resources_path / "complex_flex.xml", "rb") as f:
            mock_fetch.return_value = f.read()

        # 1. Fetch XML
        runner.invoke(ibkr_porez, ["get"])

        # 2. Import CSV
        result = runner.invoke(ibkr_porez, ["import", str(resources_path / "complex_activity.csv")])

        # Expected:
        # csv-AAPL (Jan 1) -> Skipped (XML covers Jan 1)
        # csv-Divs (Mar 15) -> Skipped (XML covers Mar 15)
        # csv-MSFT (Apr 1) -> Added (XML doesn't cover Apr 1)

        assert result.exit_code == 0

        s = Storage()
        txs = s.get_transactions()

        # Total: 7 XML + 1 CSV (MSFT) = 8
        assert len(txs) == 8

        # Stats in output?
        # New: 1 (MSFT).
        # assert "Parsed 4 transactions"
        # assert "(1 new" in result.output

        assert "Parsed 4 transactions" in result.output
        assert "(1 new" in result.output

    @patch("ibkr_porez.operation_import.NBSClient")
    @patch("ibkr_porez.main.config_manager")
    def test_import_invalid_file(self, mock_cfg_mgr, mock_nbs_cls, runner, mock_user_data_dir):
        mock_cfg_mgr.load_config.return_value = MagicMock()
        mock_nbs = mock_nbs_cls.return_value
        mock_nbs.get_rate.return_value = None

        with runner.isolated_filesystem():
            with open("broken.csv", "w") as f:
                f.write("Just garbage data")

            result = runner.invoke(ibkr_porez, ["import", "broken.csv"])

            # CLI catches exception and prints message or parser returns empty.
            # Code says: "No valid transactions found in file."
            assert result.exit_code == 0
            assert "No valid transactions found" in result.output
