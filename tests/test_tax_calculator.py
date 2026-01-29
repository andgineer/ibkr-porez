from datetime import date
from decimal import Decimal
from unittest.mock import patch, MagicMock
import pandas as pd

from ibkr_porez.models import Transaction, TransactionType, Currency
from ibkr_porez.nbs import NBSClient
from ibkr_porez.tax import TaxCalculator


class MockNBSClient(NBSClient):
    def __init__(self):
        self.rates = {}

    def get_rate(self, date_obj, currency):
        key = (date_obj, currency)
        return self.rates.get(key)

    def add_rate(self, date_obj, currency, rate):
        self.rates[(date_obj, currency)] = Decimal(str(rate))


def test_tax_calculator_fifo_profit():
    nbs = MockNBSClient()
    nbs.add_rate(date(2023, 1, 1), Currency.USD, "100.0")
    nbs.add_rate(date(2023, 6, 1), Currency.USD, "110.0")

    tax_calc = TaxCalculator(nbs)

    # 1. Buy 10 AAPL @ $100 on Jan 1
    buy = Transaction(
        transaction_id="1",
        date=date(2023, 1, 1),
        type=TransactionType.TRADE,
        symbol="AAPL",
        description="Buy",
        quantity=Decimal("10"),
        price=Decimal("100"),
        amount=Decimal("-1000"),
        currency=Currency.USD,
    )

    # 2. Sell 10 AAPL @ $150 on Jun 1
    sale = Transaction(
        transaction_id="2",
        date=date(2023, 6, 1),
        type=TransactionType.TRADE,
        symbol="AAPL",
        description="Sell",
        quantity=Decimal("-10"),
        price=Decimal("150"),  # Sale $150
        amount=Decimal("1500"),
        currency=Currency.USD,
    )

    data = [buy.model_dump(mode="json"), sale.model_dump(mode="json")]
    df = pd.DataFrame(data)
    # Ensure types for test mock
    df["date"] = pd.to_datetime(df["date"]).dt.date

    entries = tax_calc.process_trades(df)
    assert len(entries) == 1
    entry = entries[0]

    assert entry.quantity == Decimal("10")

    # Sale Value: 10 * 150 * 110 = 165000
    assert entry.sale_value_rsd == Decimal("165000.00")

    # Purchase Value: 10 * 100 * 100 = 100000
    assert entry.purchase_value_rsd == Decimal("100000.00")

    # Gain: 65000
    assert entry.capital_gain_rsd == Decimal("65000.00")
    assert not entry.is_tax_exempt


def test_tax_calculator_fifo_split_lot():
    """Test selling 5 shares from a lot of 10."""
    nbs = MockNBSClient()
    nbs.add_rate(date(2023, 1, 1), Currency.USD, "100.0")
    nbs.add_rate(date(2023, 6, 1), Currency.USD, "110.0")

    tax_calc = TaxCalculator(nbs)

    # Buy 10
    buy = Transaction(
        transaction_id="1",
        date=date(2023, 1, 1),
        type=TransactionType.TRADE,
        symbol="AAPL",
        description="Buy",
        quantity=Decimal("10"),
        price=Decimal("100"),
        amount=Decimal("-1000"),
        currency=Currency.USD,
    )

    # Sell 4
    sale = Transaction(
        transaction_id="2",
        date=date(2023, 6, 1),
        type=TransactionType.TRADE,
        symbol="AAPL",
        description="Sell",
        quantity=Decimal("-4"),
        price=Decimal("150"),
        amount=Decimal("600"),
        currency=Currency.USD,
    )

    df = pd.DataFrame([buy.model_dump(mode="json"), sale.model_dump(mode="json")])
    df["date"] = pd.to_datetime(df["date"]).dt.date

    entries = tax_calc.process_trades(df)
    assert len(entries) == 1
    entry = entries[0]

    assert entry.quantity == Decimal("4")
    # Purchase Value: 4 * 100 * 100 = 40000
    assert entry.purchase_value_rsd == Decimal("40000.00")


def test_tax_calculator_fifo_multi_lot():
    """Test selling 15 shares covering 2 lots (10 @ 100, 10 @ 200)."""
    nbs = MockNBSClient()
    nbs.add_rate(date(2023, 1, 1), Currency.USD, "100.0")
    nbs.add_rate(date(2023, 2, 1), Currency.USD, "100.0")
    nbs.add_rate(date(2023, 6, 1), Currency.USD, "100.0")

    tax_calc = TaxCalculator(nbs)

    # Buy 10 @ 100
    buy1 = Transaction(
        transaction_id="1",
        date=date(2023, 1, 1),
        type=TransactionType.TRADE,
        symbol="AAPL",
        description="Buy",
        quantity=Decimal("10"),
        price=Decimal("100"),
        amount=Decimal("-1000"),
        currency=Currency.USD,
    )
    # Buy 10 @ 200
    buy2 = Transaction(
        transaction_id="2",
        date=date(2023, 2, 1),
        type=TransactionType.TRADE,
        symbol="AAPL",
        description="Buy",
        quantity=Decimal("10"),
        price=Decimal("200"),
        amount=Decimal("-2000"),
        currency=Currency.USD,
    )

    # Sell 15
    sale = Transaction(
        transaction_id="3",
        date=date(2023, 6, 1),
        type=TransactionType.TRADE,
        symbol="AAPL",
        description="Sell",
        quantity=Decimal("-15"),
        price=Decimal("300"),
        amount=Decimal("4500"),
        currency=Currency.USD,
    )

    df = pd.DataFrame(
        [buy1.model_dump(mode="json"), buy2.model_dump(mode="json"), sale.model_dump(mode="json")]
    )
    df["date"] = pd.to_datetime(df["date"]).dt.date

    entries = tax_calc.process_trades(df)
    assert len(entries) == 2

    # Entry 1: 10 shares from Lot 1
    e1 = entries[0]
    assert e1.quantity == Decimal("10")
    assert e1.purchase_price == Decimal("100")

    # Entry 2: 5 shares from Lot 2
    e2 = entries[1]
    assert e2.quantity == Decimal("5")
    assert e2.purchase_price == Decimal("200")


@patch("requests.get")
def test_nbs_fallback(mock_get):
    from ibkr_porez.nbs import NBSClient, Currency

    storage = MagicMock()
    storage.get_exchange_rate.return_value = None

    nbs = NBSClient(storage)

    # Mock sequence with holidays calculation:
    # 1. 2023-01-01 (Sunday) -> Skip (Weekend/Holiday)
    # 2. 2022-12-31 (Saturday) -> Skip (Weekend)
    # 3. 2022-12-30 (Friday) -> Fetch (Working Day) -> 200 OK

    mock_resp_200 = MagicMock()
    mock_resp_200.json.return_value = {"exchange_middle": 117.5}
    mock_resp_200.raise_for_status.return_value = None

    mock_get.side_effect = [mock_resp_200]

    rate = nbs.get_rate(date(2023, 1, 1), Currency.EUR)

    assert rate == Decimal("117.5")
    # Should only call API ONCE for the Friday
    assert mock_get.call_count == 1

    # Verify save is called TWICE
    assert storage.save_exchange_rate.call_count == 2
