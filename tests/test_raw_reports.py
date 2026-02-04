"""Tests for raw reports delta storage and restoration."""

from datetime import date


from ibkr_porez.raw_reports import restore_report, save_raw_report_with_delta
from ibkr_porez.storage import Storage


def test_save_first_report_as_base(tmp_path, monkeypatch):
    """Test that first report is saved as base file."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    xml_content = '<?xml version="1.0"?><root><data>test</data></root>'
    report_date = date(2026, 1, 29)

    save_raw_report_with_delta(storage, xml_content, report_date)

    base_file = storage._flex_queries_dir / "base_20260129.xml"
    assert base_file.exists()
    assert base_file.read_text(encoding="utf-8") == xml_content


def test_save_second_report_as_delta(tmp_path, monkeypatch):
    """Test that second report is saved (as delta or new base if delta too large)."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    # Use larger XML files with minimal changes (like real IBKR reports)
    # Real reports add new transactions, so delta is small
    base_xml = (
        '<?xml version="1.0"?><root>'
        + '<transaction id="1"/><transaction id="2"/>' * 500
        + "</root>"
    )
    # Add only one new transaction (minimal change)
    new_xml = (
        '<?xml version="1.0"?><root>'
        + '<transaction id="1"/><transaction id="2"/>' * 500
        + '<transaction id="3"/></root>'
    )

    save_raw_report_with_delta(storage, base_xml, date(2026, 1, 29))
    save_raw_report_with_delta(storage, new_xml, date(2026, 1, 30))

    # Either delta exists, or new base exists (if delta was too large)
    delta_file = storage._flex_queries_dir / "delta_20260130.patch"
    new_base_file = storage._flex_queries_dir / "base_20260130.xml"

    # At least one should exist
    assert delta_file.exists() or new_base_file.exists()

    # If delta exists, it should contain diff
    if delta_file.exists():
        delta_content = delta_file.read_text(encoding="utf-8")
        assert "transaction" in delta_content


def test_restore_report_from_base(tmp_path, monkeypatch):
    """Test restoring report when only base exists."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    xml_content = '<?xml version="1.0"?><root><data>test</data></root>'
    report_date = date(2026, 1, 29)

    save_raw_report_with_delta(storage, xml_content, report_date)

    restored = restore_report(storage, report_date)
    assert restored == xml_content


def test_restore_report_with_deltas(tmp_path, monkeypatch):
    """Test restoring report by applying deltas."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    base_xml = '<?xml version="1.0"?><root><data>base</data></root>'
    final_xml = '<?xml version="1.0"?><root><data>final</data><extra>added</extra></root>'

    save_raw_report_with_delta(storage, base_xml, date(2026, 1, 29))
    save_raw_report_with_delta(storage, final_xml, date(2026, 1, 30))

    # Restore the final report
    restored = restore_report(storage, date(2026, 1, 30))
    assert restored is not None
    # Should contain the final content
    assert "final" in restored
    assert "added" in restored


def test_restore_nonexistent_report(tmp_path, monkeypatch):
    """Test restoring report that doesn't exist."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    restored = restore_report(storage, date(2026, 1, 1))
    assert restored is None


def test_large_delta_creates_new_base(tmp_path, monkeypatch):
    """Test that large delta (>50% of base) creates new base file."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    # Small base
    base_xml = '<?xml version="1.0"?><root><data>small</data></root>'
    # Large new XML (much more than 50% of base)
    large_xml = '<?xml version="1.0"?><root><data>large</data>' + "x" * 1000 + "</root>"

    save_raw_report_with_delta(storage, base_xml, date(2026, 1, 29))
    save_raw_report_with_delta(storage, large_xml, date(2026, 1, 30))

    # Should create new base instead of large delta
    new_base = storage._flex_queries_dir / "base_20260130.xml"
    assert new_base.exists()
    assert new_base.read_text(encoding="utf-8") == large_xml


def test_restore_multiple_deltas(tmp_path, monkeypatch):
    """Test restoring report with multiple sequential deltas."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    # Use larger XML files with minimal changes (like real IBKR reports)
    xml1 = '<?xml version="1.0"?><root>' + '<transaction id="1"/>' * 500 + "</root>"
    xml2 = (
        '<?xml version="1.0"?><root>'
        + '<transaction id="1"/>' * 500
        + '<transaction id="2"/></root>'
    )
    xml3 = (
        '<?xml version="1.0"?><root>'
        + '<transaction id="1"/>' * 500
        + '<transaction id="2"/><transaction id="3"/></root>'
    )

    save_raw_report_with_delta(storage, xml1, date(2026, 1, 29))
    save_raw_report_with_delta(storage, xml2, date(2026, 1, 30))
    save_raw_report_with_delta(storage, xml3, date(2026, 1, 31))

    # Restore intermediate (may be None if new base was created)
    restored_30 = restore_report(storage, date(2026, 1, 30))
    if restored_30 is not None:
        assert 'transaction id="2"' in restored_30

    # Restore final
    restored_31 = restore_report(storage, date(2026, 1, 31))
    assert restored_31 is not None
    assert 'transaction id="3"' in restored_31


def test_restore_file_from_delta(tmp_path, monkeypatch):
    """Test that we can actually restore the original file from base + delta."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    # Create original XML with each trade on separate line for smaller delta
    original_xml = (
        '<?xml version="1.0"?>\n<root>\n'
        + '<Trade symbol="AAPL" date="2025-12-23" />\n'
        + '<Trade symbol="MSFT" date="2025-12-26" />\n'
        + '<Trade symbol="GOOGL" date="2026-01-15" />\n'
        + "</root>\n"
    )

    # Modified XML (add one trade)
    modified_xml = (
        '<?xml version="1.0"?>\n<root>\n'
        + '<Trade symbol="AAPL" date="2025-12-23" />\n'
        + '<Trade symbol="MSFT" date="2025-12-26" />\n'
        + '<Trade symbol="GOOGL" date="2026-01-15" />\n'
        + '<Trade symbol="TSLA" date="2026-01-20" />\n'
        + "</root>\n"
    )

    # Save original as base
    save_raw_report_with_delta(storage, original_xml, date(2026, 1, 29))

    # Save modified as delta
    save_raw_report_with_delta(storage, modified_xml, date(2026, 1, 30))

    # Restore the modified file from base + delta
    restored = restore_report(storage, date(2026, 1, 30))

    # Verify restored content matches the original modified XML exactly
    assert restored is not None
    assert restored == modified_xml, (
        f"Restored content doesn't match. Got:\n{restored}\nExpected:\n{modified_xml}"
    )
    assert "TSLA" in restored
    assert "AAPL" in restored
    assert "MSFT" in restored
    assert "GOOGL" in restored

    # Verify that we can restore the base file (before delta was applied)
    base_file = storage._flex_queries_dir / "base_20260129.xml"
    delta_file = storage._flex_queries_dir / "delta_20260130.patch"
    new_base_file = storage._flex_queries_dir / "base_20260130.xml"

    if delta_file.exists() and base_file.exists():
        # Delta was small, base still exists - verify base content
        base_content = base_file.read_text(encoding="utf-8")
        assert base_content == original_xml
        assert "TSLA" not in base_content
    elif new_base_file.exists():
        # Delta was large, new base was created - verify it contains modified content
        new_base_content = new_base_file.read_text(encoding="utf-8")
        assert new_base_content == modified_xml
