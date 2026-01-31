import allure
import io
import pytest
from datetime import date
from decimal import Decimal
from ibkr_porez.models import Currency, TransactionType
from ibkr_porez.parsers.csv_parser import CSVParser


@pytest.fixture
def sample_csv():
    # A CSV mimicking IBKR Activity Statement structure
    # Note: Using different sections and ensuring headers are parsed
    return """
"Statement","Header","DataDiscriminator","Asset Category","Currency","Symbol","Date/Time","Quantity","T. Price","Proceeds","Comm/Fee","Basis","Realized P/L","Mtm P/L","Code","Transaction ID"
"Trades","Header","Data","Asset Category","Currency","Symbol","Date/Time","Quantity","T. Price","Proceeds","Comm/Fee","Basis","Realized P/L","Mtm P/L","Code","Transaction ID"
"Trades","Data","Order","Stocks","USD","AAPL","2023-01-05","10","150","1500","-1","1500","0","0","O","TX1001"
"Trades","Data","Order","Stocks","USD","MSFT","2023-01-10","-5","300","-1500","-1","1400","100","0","C","TX1002"
"Dividends","Header","Data","Asset Category","Currency","Symbol","Date","Amount","Description","Transaction ID"
"Dividends","Data","Data","Stocks","USD","KO","2023-01-15","50","Dividend Payment","TX2001"
"Withholding Tax","Header","Data","Asset Category","Currency","Symbol","Date","Amount","Description","Transaction ID"
"Withholding Tax","Data","Data","Stocks","USD","KO","2023-01-15","-7.5","Tax","TX2002"
"""


@allure.epic("IBKR")
@allure.feature("Import CSV")
class TestCSVParser:
    def test_parse_csv_trades(self, sample_csv):
        parser = CSVParser()
        f = io.StringIO(sample_csv)
        transactions = parser.parse(f)

        # Filter trades
        trades = [t for t in transactions if t.type == TransactionType.TRADE]
        assert len(trades) == 2

        t1 = next(t for t in trades if t.symbol == "AAPL")
        assert t1.date == date(2023, 1, 5)
        assert t1.quantity == Decimal("10")
        assert t1.price == Decimal("150")
        assert t1.amount == Decimal("1500")
        assert t1.transaction_id == "TX1001"
        assert t1.currency == Currency.USD

        t2 = next(t for t in trades if t.symbol == "MSFT")
        assert t2.date == date(2023, 1, 10)
        assert t2.quantity == Decimal("-5")
        assert t2.price == Decimal("300")
        assert t2.amount == Decimal("-1500")
        assert t2.transaction_id == "TX1002"

    def test_parse_csv_dividends(self, sample_csv):
        parser = CSVParser()
        f = io.StringIO(sample_csv)
        transactions = parser.parse(f)

        divs = [t for t in transactions if t.type == TransactionType.DIVIDEND]
        assert len(divs) == 1
        d1 = divs[0]
        assert d1.symbol == "KO"
        assert d1.amount == Decimal("50")
        assert d1.date == date(2023, 1, 15)
        assert d1.transaction_id == "TX2001"

    def test_parse_csv_withholding_tax(self, sample_csv):
        parser = CSVParser()
        f = io.StringIO(sample_csv)
        transactions = parser.parse(f)

        taxes = [t for t in transactions if t.type == TransactionType.WITHHOLDING_TAX]
        assert len(taxes) == 1
        t1 = taxes[0]
        assert t1.symbol == "KO"
        assert t1.amount == Decimal("-7.5")
        assert t1.transaction_id == "TX2002"

    def test_parse_csv_missing_ids(self):
        # Test fallback ID generation when Transaction ID is missing
        csv_content = """
"Statement","Header"
"Trades","Header","DataDiscriminator","Asset Category","Currency","Symbol","Date/Time","Quantity","T. Price","Proceeds","Code"
"Trades","Data","Order","Stocks","USD","GOOG","2023-02-01, 10:00:00","5","100","500","O"
"""
        parser = CSVParser()
        f = io.StringIO(csv_content)
        transactions = parser.parse(f)

        assert len(transactions) == 1
        t = transactions[0]
        assert t.symbol == "GOOG"
        assert t.date == date(2023, 2, 1)
        # Check ID synthesis
        assert t.transaction_id == "csv-GOOG-2023-02-01, 10:00:00-5-100"
