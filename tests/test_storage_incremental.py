import pytest
import pandas as pd
from datetime import date
from ibkr_porez.storage import Storage


@pytest.fixture
def storage(tmp_path):
    # Mock user_data_dir to return tmp_path
    with pytest.MonkeyPatch.context() as m:
        m.setattr("ibkr_porez.storage.user_data_dir", lambda app: str(tmp_path))
        s = Storage()
        # Ensure dirs for test
        s._ensure_dirs()
        yield s


def test_get_last_transaction_date(storage):
    # Empty
    assert storage.get_last_transaction_date() is None

    # Add some data
    from datetime import date

    # 2023 H1
    df1 = pd.DataFrame(
        [
            {"transaction_id": "1", "date": date(2023, 1, 1), "amount": 100},
            {"transaction_id": "2", "date": date(2023, 6, 1), "amount": 200},
        ]
    )
    storage._save_partition(2023, 1, df1)

    assert storage.get_last_transaction_date() == date(2023, 6, 1)

    # 2023 H2
    df2 = pd.DataFrame(
        [
            {"transaction_id": "3", "date": date(2023, 7, 15), "amount": 100},
        ]
    )
    storage._save_partition(2023, 2, df2)

    assert storage.get_last_transaction_date() == date(2023, 7, 15)

    # 2022 H1 (Older)
    df3 = pd.DataFrame(
        [
            {"transaction_id": "0", "date": date(2022, 1, 1), "amount": 100},
        ]
    )
    storage._save_partition(2022, 1, df3)

    # Still 2023-07-15
    assert storage.get_last_transaction_date() == date(2023, 7, 15)


def test_get_transactions_open_date_conversion(storage):
    # Test that open_date is converted to date object
    import pandas as pd

    df = pd.DataFrame(
        [
            {
                "transaction_id": "100",
                "date": date(2023, 1, 1),
                "open_date": "2022-06-01",  # String
                "amount": 100,
                "currency": "USD",
            },
        ]
    )
    storage._save_partition(2023, 1, df)

    loaded_df = storage.get_transactions()

    assert not loaded_df.empty
    row = loaded_df.iloc[0]
    assert isinstance(row["date"], date)
    assert isinstance(row["open_date"], date)
    assert str(row["open_date"]) == "2022-06-01"
