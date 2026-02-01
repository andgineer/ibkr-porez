import allure
import pytest
from unittest.mock import MagicMock, patch
from ibkr_porez.ibkr_flex_query import IBKRClient
from ibkr_porez.models import TransactionType


@pytest.fixture
def sample_xml_report():
    return b"""
    <FlexQueryResponse>
        <FlexStatements>
            <FlexStatement>
                <Trades>
                    <Trade symbol="AAPL" currency="USD" quantity="10" tradePrice="150.0" 
                           tradeDate="20230101" tradeID="123456" 
                           subCategory="Stock" extraField="ignoreMe"
                           origTradeDate="20220601" origTradePrice="120.0"
                           fifoPnlRealized="300.0" />
                </Trades>
                <CashTransactions>
                    <CashTransaction type="Dividends" symbol="AAPL" amount="5.0" currency="USD" 
                                     dateTime="20230115;10:00:00" transactionID="987654" 
                                     ignoredAttr="val" />
                </CashTransactions>
            </FlexStatement>
        </FlexStatements>
    </FlexQueryResponse>
    """


@allure.epic("IBKR")
@allure.feature("Flex Query")
class TestIBKRClient:
    def test_parse_report(self, sample_xml_report):
        client = IBKRClient("token", "query")
        transactions = client.parse_report(sample_xml_report)

        assert len(transactions) == 2

        # Check Trade
        trade = transactions[0]
        assert trade.type == TransactionType.TRADE
        assert trade.symbol == "AAPL"
        assert trade.quantity == 10
        assert trade.price == 150.0
        assert trade.amount == 300.0  # fifoPnlRealized
        assert str(trade.date) == "2023-01-01"

        # Check Cash Transaction
        div = transactions[1]
        assert div.type == TransactionType.DIVIDEND
        assert div.symbol == "AAPL"
        assert div.amount == 5.0
        assert str(div.date) == "2023-01-15"

    @patch("requests.get")
    @patch("time.sleep")
    def test_fetch_latest_report(self, mock_sleep, mock_get):
        client = IBKRClient("token", "query")

        # Mock Step 1
        mock_resp1 = MagicMock()
        mock_resp1.content = b"<FlexStatementResponse><ReferenceCode>REF123</ReferenceCode><Url>http://test.com</Url></FlexStatementResponse>"

        # Mock Step 2
        mock_resp2 = MagicMock()
        mock_resp2.content = b"<Report>Data</Report>"

        mock_get.side_effect = [mock_resp1, mock_resp2]

        data = client.fetch_latest_report()

        assert data == b"<Report>Data</Report>"
        assert mock_get.call_count == 2

        # Verify Step 1 params
        args1, kwargs1 = mock_get.call_args_list[0]
        assert kwargs1["params"]["q"] == "query"

        # Verify Step 2 params
        args2, kwargs2 = mock_get.call_args_list[1]
        assert kwargs2["params"]["q"] == "REF123"

    def test_parse_report_error(self):
        client = IBKRClient("token", "query")
        error_xml = b"<FlexStatementResponse><Status>Error</Status><ErrorCode>1012</ErrorCode><ErrorMessage>Invalid Token</ErrorMessage></FlexStatementResponse>"

        with pytest.raises(ValueError, match="Flex Query Failed: 1012 - Invalid Token"):
            client.parse_report(error_xml)
