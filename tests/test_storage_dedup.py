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

    inserted, updated = storage.save_transactions([t1])
    assert inserted == 1
    assert updated == 0

    # Save same - strict match
    inserted, updated = storage.save_transactions([t1_updated])
    assert inserted == 0
    assert updated == 1
    stored = storage.get_transactions()
    assert len(stored) == 1
    assert stored.iloc[0]["description"] == "Updated"


def test_dedup_xml_upgrades_csv(storage):
    # Scenario A: New XML (get) meets Existing CSV (import)
    # Existing CSV has synthesized ID
    t_csv = make_tx("csv-AAPL-...", "2023-01-05", "AAPL", "5", "100.00001")
    inserted, updated = storage.save_transactions([t_csv])
    assert inserted == 1
    assert updated == 0

    # New XML has official ID and matchable semantic data (fuzzy price)
    t_xml = make_tx("OFFICIAL_ID", "2023-01-05", "AAPL", "5", "100.0")

    # Save XML - should REPLACE CSV
    # Upgrade adds to `to_add` AND `ids_to_remove`.
    # `updates_count` only increments on STRICT ID match.
    # So `inserted_count` = len(to_add) - updates_count.
    # len(to_add) is 1. updates_count is 0.
    # So inserted is 1. Updated is 0.
    # Wait, user sees it as "1 new"?
    # Ideally "Updated". But for now let's assert what logical code does.
    # Upgrade = New Record (technically).
    # Update: Logic changed to count Upgrade as Update for user clarity.
    inserted, updated = storage.save_transactions([t_xml])
    assert inserted == 0
    assert updated == 1

    stored = storage.get_transactions()
    assert len(stored) == 1
    assert stored.iloc[0]["transaction_id"] == "OFFICIAL_ID"


def test_dedup_csv_skips_xml(storage):
    # Scenario B: New CSV (import) meets Existing XML (get)
    t_xml = make_tx("OFFICIAL_ID", "2023-01-05", "AAPL", "5", "100.0")
    inserted, updated = storage.save_transactions([t_xml])
    assert inserted == 1
    assert updated == 0

    t_csv = make_tx("csv-AAPL-...", "2023-01-05", "AAPL", "5", "100.00001")

    # Save CSV - should be SKIPPED
    inserted, updated = storage.save_transactions([t_csv])
    assert inserted == 0
    assert updated == 0

    stored = storage.get_transactions()
    assert len(stored) == 1
    assert stored.iloc[0]["transaction_id"] == "OFFICIAL_ID"


def test_dedup_split_orders_counter(storage):
    # Scenario: 2 identical trades on same day
    t_csv_1 = make_tx("csv-1", "2023-02-01", "IBKR", "10", "50")
    t_csv_2 = make_tx("csv-2", "2023-02-01", "IBKR", "10", "50")

    inserted, updated = storage.save_transactions([t_csv_1, t_csv_2])
    assert inserted == 2
    assert updated == 0
    assert len(storage.get_transactions()) == 2

    # New XML come in (official IDs)
    t_xml_1 = make_tx("xml-1", "2023-02-01", "IBKR", "10", "50")

    # Save 1 XML - should replace 1 CSV, leave 1 CSV
    # (Assuming we process 1 by 1 or batch? Storage logic handles batch)
    inserted, updated = storage.save_transactions([t_xml_1])
    assert inserted == 0
    assert updated == 1

    stored = storage.get_transactions()
    # XML Supremacy: 1 XML overwrites ALL CSVs for that day.
    # So we expect 1 record (the XML one).
    stored = storage.get_transactions()
    assert len(stored) == 1
    assert stored.iloc[0]["transaction_id"] == "xml-1"

    # Save 2nd XML
    t_xml_2 = make_tx("xml-2", "2023-02-01", "IBKR", "10", "50")
    inserted, updated = storage.save_transactions([t_xml_2])
    assert inserted == 1
    assert updated == 0

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
    inserted, updated = storage.save_transactions([t_xml_1, t_xml_2])
    assert inserted == 2
    assert updated == 0

    # 2. Incoming CSV (Bundle) - different quantity, so no semantic match!
    t_csv_bundle = make_tx("csv-bundle", "2025-12-23", "IJH", "88", "50")

    # Save CSV
    inserted, updated = storage.save_transactions([t_csv_bundle])
    assert inserted == 0
    assert updated == 0

    # Verify: Should ONLY have XML records. CSV bundle skipped due to date coverage.
    stored = storage.get_transactions()
    assert len(stored) == 2
    ids = set(stored["transaction_id"])
    assert "xml-1" in ids
    assert "xml-2" in ids
    assert "csv-bundle" not in ids


def test_xml_supremacy_reverse_order(storage):
    # Scenario: "Reverse Order"
    # 1. User Imports CSV first (bundled trade)
    # 2. User then runs GET (XML split trades)
    # Result: The XML should DETECT that it covers the date, and DELETE the CSV record.

    # 1. Import CSV (Bundle)
    t_csv = make_tx("csv-bundle", "2025-12-23", "IJH", "88", "50")
    inserted, updated = storage.save_transactions([t_csv])
    assert inserted == 1
    assert updated == 0

    # Verify CSV is there
    stored = storage.get_transactions()
    assert len(stored) == 1
    assert stored.iloc[0]["transaction_id"] == "csv-bundle"

    # 2. Sync XML (Split) - Official Data arrives
    t_xml_1 = make_tx("xml-1", "2025-12-23", "IJH", "77", "50")
    t_xml_2 = make_tx("xml-2", "2025-12-23", "IJH", "11", "50")

    inserted, updated = storage.save_transactions([t_xml_1, t_xml_2])
    assert inserted == 2
    assert updated == 0

    # Verify: CSV is GONE. XMLs are present.
    stored = storage.get_transactions()
    assert len(stored) == 2
    ids = set(stored["transaction_id"])
    assert "xml-1" in ids
    assert "xml-2" in ids
    assert "csv-bundle" not in ids


def test_dedup_id_type_mismatch(storage):
    # Scenario: DataFrame loads ID as int, new ID is str.
    # Without fix, "123" (str) in [123 (int)] is False, causing duplication.
    # With fix, storage converts loaded IDs to strings.

    # 1. Create a file with INT ids
    import pandas as pd

    # We must ensure we write a CLEAN int file
    df = pd.DataFrame(
        [
            {
                "transaction_id": 12345,
                "date": "2025-01-01",
                "type": "BUY",
                "symbol": "AAPL",
                "quantity": 10,
                "price": 100,
                "amount": 1000,
                "currency": "USD",
            }
        ]
    )

    p = storage._partition_dir / "transactions_2025_H1.json"
    p.parent.mkdir(exist_ok=True, parents=True)
    df.to_json(p, orient="records")

    # 2. Try to save SAME transaction but with string ID
    t_new = make_tx("12345", "2025-01-01", "AAPL", "10", "100")

    # 3. Save
    inserted, updated = storage.save_transactions([t_new])

    # Expect 0 inserted, 1 updated (strict ID match)
    assert inserted == 0
    assert updated == 1
    stored = storage.get_transactions()
    assert len(stored) == 1
    # pd.read_json might reload "12345" as int, but as long as it's deduplicated (len=1), we are good.
    assert str(stored.iloc[0]["transaction_id"]) == "12345"


def test_dedup_identical_skip(storage):
    # Scenario: Exact duplicate processing
    # If we save t1, then save t1 again (unchanged), it should be skipped ENTIRELY.
    # Count: 0 inserted, 0 updated.

    t1 = make_tx("ID_PERFECT", "2023-01-01", "AAPL", "10", "150.0")

    # 1. First Save
    inserted, updated = storage.save_transactions([t1])
    assert inserted == 1
    assert updated == 0

    # 2. Second Save (Identical)
    inserted, updated = storage.save_transactions([t1])
    assert inserted == 0
    assert updated == 0  # Should be 0, not 1!

    stored = storage.get_transactions()
    assert len(stored) == 1
