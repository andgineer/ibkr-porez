import allure
import pytest
from datetime import date
from decimal import Decimal

from ibkr_porez.models import Currency, Transaction, TransactionType, UserConfig
from ibkr_porez.storage import Storage


@pytest.fixture
def storage(tmp_path):
    # Mock user_data_dir and config_manager to return tmp_path
    with pytest.MonkeyPatch.context() as m:
        m.setattr("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path))
        mock_config = UserConfig(full_name="Test", address="Test", data_dir=None)
        m.setattr("ibkr_porez.storage.config_manager.load_config", lambda: mock_config)
        s = Storage()
        # Ensure dirs for test
        s._ensure_dirs()
        yield s


@allure.epic("Storage")
@allure.feature("Incremental Loading")
class TestStorageIncremental:
    def test_get_last_transaction_date(self, storage):
        # Empty
        assert storage.get_last_transaction_date() is None

        # Add some data
        # Create transactions and save them
        tx1 = Transaction(
            transaction_id="1",
            date=date(2023, 1, 1),
            type=TransactionType.TRADE,
            symbol="AAPL",
            description="test",
            quantity=Decimal("10"),
            price=Decimal("100"),
            amount=Decimal("1000"),
            currency=Currency.USD,
        )
        tx2 = Transaction(
            transaction_id="2",
            date=date(2023, 6, 1),
            type=TransactionType.TRADE,
            symbol="AAPL",
            description="test",
            quantity=Decimal("20"),
            price=Decimal("100"),
            amount=Decimal("2000"),
            currency=Currency.USD,
        )
        storage.save_transactions([tx1, tx2])

        assert storage.get_last_transaction_date() == date(2023, 6, 1)

        # Add more transactions
        tx3 = Transaction(
            transaction_id="3",
            date=date(2023, 7, 15),
            type=TransactionType.TRADE,
            symbol="AAPL",
            description="test",
            quantity=Decimal("15"),
            price=Decimal("100"),
            amount=Decimal("1500"),
            currency=Currency.USD,
        )
        storage.save_transactions([tx3])

        assert storage.get_last_transaction_date() == date(2023, 7, 15)

        # Add older transaction
        tx4 = Transaction(
            transaction_id="0",
            date=date(2022, 1, 1),
            type=TransactionType.TRADE,
            symbol="AAPL",
            description="test",
            quantity=Decimal("5"),
            price=Decimal("100"),
            amount=Decimal("500"),
            currency=Currency.USD,
        )
        storage.save_transactions([tx4])

        # Still 2023-07-15
        assert storage.get_last_transaction_date() == date(2023, 7, 15)

    def test_get_transactions_open_date_conversion(self, storage):
        # Test that open_date is converted to date object
        tx = Transaction(
            transaction_id="100",
            date=date(2023, 1, 1),
            open_date=date(2022, 6, 1),
            type=TransactionType.TRADE,
            symbol="AAPL",
            description="test",
            quantity=Decimal("10"),
            price=Decimal("100"),
            amount=Decimal("1000"),
            currency=Currency.USD,
        )
        storage.save_transactions([tx])

        loaded_df = storage.get_transactions()

        assert not loaded_df.empty
        row = loaded_df.iloc[0]
        assert isinstance(row["date"], date)
        assert isinstance(row["open_date"], date)
        assert str(row["open_date"]) == "2022-06-01"
