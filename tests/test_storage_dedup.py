from datetime import date
from decimal import Decimal
import pytest
from ibkr_porez.models import Transaction, TransactionType, Currency
from ibkr_porez.storage import Storage


@pytest.fixture
def storage(tmp_path):
    s = Storage()
    # Mock data dir
    s._data_dir = tmp_path
    s._partition_dir = tmp_path / "partitions"
    s._ensure_dirs()
    return s


def make_tx(tx_id, d_str, symbol, qty, price, t_type=TransactionType.TRADE):
    return Transaction(
        transaction_id=tx_id,
        date=date.fromisoformat(d_str),
        type=t_type,
        symbol=symbol,
        description="test",
        quantity=Decimal(qty),
        price=Decimal(price),
        amount=Decimal(qty) * Decimal(price),
        currency=Currency.USD,
    )


def test_dedup_strict_id_match(storage):
    # Scenario: Same ID updates existing record
    t1 = make_tx("ID1", "2023-01-01", "AAPL", "10", "150.0")
    t1_updated = make_tx("ID1", "2023-01-01", "AAPL", "10", "150.0")
    t1_updated.description = "Updated"

    storage.save_transactions([t1])
    assert len(storage.get_transactions()) == 1

    storage.save_transactions([t1_updated])
    stored = storage.get_transactions()
    assert len(stored) == 1
    assert stored.iloc[0]["description"] == "Updated"


def test_dedup_xml_upgrades_csv(storage):
    # Scenario A: New XML (get) meets Existing CSV (import)
    # Existing CSV has synthesized ID
    t_csv = make_tx("csv-AAPL-...", "2023-01-05", "AAPL", "5", "100.00001")
    storage.save_transactions([t_csv])

    # New XML has official ID and matchable semantic data (fuzzy price)
    t_xml = make_tx("OFFICIAL_ID", "2023-01-05", "AAPL", "5", "100.0")

    # Save XML - should REPLACE CSV
    storage.save_transactions([t_xml])

    stored = storage.get_transactions()
    assert len(stored) == 1
    assert stored.iloc[0]["transaction_id"] == "OFFICIAL_ID"


def test_dedup_csv_skips_xml(storage):
    # Scenario B: New CSV (import) meets Existing XML (get)
    t_xml = make_tx("OFFICIAL_ID", "2023-01-05", "AAPL", "5", "100.0")
    storage.save_transactions([t_xml])

    t_csv = make_tx("csv-AAPL-...", "2023-01-05", "AAPL", "5", "100.00001")

    # Save CSV - should be SKIPPED
    storage.save_transactions([t_csv])

    stored = storage.get_transactions()
    assert len(stored) == 1
    assert stored.iloc[0]["transaction_id"] == "OFFICIAL_ID"


def test_dedup_split_orders_counter(storage):
    # Scenario: 2 identical trades on same day
    t_csv_1 = make_tx("csv-1", "2023-02-01", "IBKR", "10", "50")
    t_csv_2 = make_tx("csv-2", "2023-02-01", "IBKR", "10", "50")

    storage.save_transactions([t_csv_1, t_csv_2])
    assert len(storage.get_transactions()) == 2

    # New XML come in (official IDs)
    t_xml_1 = make_tx("xml-1", "2023-02-01", "IBKR", "10", "50")

    # Save 1 XML - should replace 1 CSV, leave 1 CSV
    # (Assuming we process 1 by 1 or batch? Storage logic handles batch)
    storage.save_transactions([t_xml_1])

    stored = storage.get_transactions()
    assert len(stored) == 2
    ids = set(stored["transaction_id"])
    assert "xml-1" in ids
    # One of the csv IDs should remain
    assert ("csv-1" in ids) or ("csv-2" in ids)

    # Save 2nd XML
    t_xml_2 = make_tx("xml-2", "2023-02-01", "IBKR", "10", "50")
    storage.save_transactions([t_xml_2])

    stored = storage.get_transactions()
    assert len(stored) == 2
    ids = set(stored["transaction_id"])
    assert "xml-1" in ids
    assert "xml-2" in ids


def test_dedup_bundle_vs_split_coverage(storage):
    # Scenario: XML has split trades (e.g. 77 and 11 qty) on 2025-12-23
    # CSV has aggregated bundled trade (88 qty) on same date
    # Result: CSV should be skipped because XML is present for this date.

    # 1. Existing XML (Split)
    t_xml_1 = make_tx("xml-1", "2025-12-23", "IJH", "77", "50")
    t_xml_2 = make_tx("xml-2", "2025-12-23", "IJH", "11", "50")
    storage.save_transactions([t_xml_1, t_xml_2])

    # 2. Incoming CSV (Bundle) - different quantity, so no semantic match!
    t_csv_bundle = make_tx("csv-bundle", "2025-12-23", "IJH", "88", "50")

    # Save CSV
    storage.save_transactions([t_csv_bundle])

    # Verify: Should ONLY have XML records. CSV bundle skipped due to date coverage.
    stored = storage.get_transactions()
    assert len(stored) == 2
    ids = set(stored["transaction_id"])
    assert "xml-1" in ids
    assert "xml-2" in ids
    assert "csv-bundle" not in ids
