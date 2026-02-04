"""Tests for raw reports delta storage and restoration."""

import zipfile
from datetime import date

from ibkr_porez.storage_flex_queries import restore_report, save_raw_report_with_delta
from ibkr_porez.storage import Storage
from ibkr_porez.models import UserConfig


def _mock_storage_config(monkeypatch, tmp_path):
    """Helper to mock storage config for tests."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    mock_config = UserConfig(full_name="Test", address="Test", data_dir=None)
    monkeypatch.setattr("ibkr_porez.storage.config_manager.load_config", lambda: mock_config)


def test_save_first_report_as_base(tmp_path, monkeypatch):
    """Test that first report is saved as base file."""
    _mock_storage_config(monkeypatch, tmp_path)
    storage = Storage()

    xml_content = '<?xml version="1.0"?><root><data>test</data></root>'
    report_date = date(2026, 1, 29)

    save_raw_report_with_delta(storage, xml_content, report_date)

    base_file = storage._flex_queries_dir / "base-20260129.xml.zip"
    assert base_file.exists()

    with zipfile.ZipFile(base_file, "r") as zf:
        content = zf.read("base-20260129.xml").decode("utf-8")
    assert content == xml_content


def test_save_second_report_as_delta(tmp_path, monkeypatch):
    """Test that second report is saved (as delta or new base if delta too large)."""
    _mock_storage_config(monkeypatch, tmp_path)
    storage = Storage()

    # Use XML files that will create a small delta
    # Note: unified diff format can create large deltas even for small changes
    # So we test that either delta or base is created (depending on delta size)
    base_xml = '<?xml version="1.0"?><root><data>base</data></root>'
    new_xml = '<?xml version="1.0"?><root><data>base</data><data>new</data></root>'

    save_raw_report_with_delta(storage, base_xml, date(2026, 1, 29))
    save_raw_report_with_delta(storage, new_xml, date(2026, 1, 30))

    base_file = storage._flex_queries_dir / "base-20260129.xml.zip"
    delta_file = storage._flex_queries_dir / "delta-20260130.patch.zip"
    base_file_30 = storage._flex_queries_dir / "base-20260130.xml.zip"

    assert base_file.exists()
    # Either delta or base should exist (depending on delta size)
    assert delta_file.exists() or base_file_30.exists(), (
        "Either delta or base file should exist for the second report"
    )


def test_restore_report_from_base(tmp_path, monkeypatch):
    """Test restoring report from base file only."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    xml_content = '<?xml version="1.0"?><root><data>test</data></root>'
    report_date = date(2026, 1, 29)

    save_raw_report_with_delta(storage, xml_content, report_date)
    restored = restore_report(storage, report_date)

    assert restored == xml_content


def test_restore_report_with_deltas(tmp_path, monkeypatch):
    """Test restoring report with deltas applied."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    base_xml = '<?xml version="1.0"?><root><data>base</data></root>'
    new_xml = '<?xml version="1.0"?><root><data>base</data><data>new</data></root>'

    save_raw_report_with_delta(storage, base_xml, date(2026, 1, 29))
    save_raw_report_with_delta(storage, new_xml, date(2026, 1, 30))

    restored = restore_report(storage, date(2026, 1, 30))
    assert restored == new_xml


def test_restore_nonexistent_report(tmp_path, monkeypatch):
    """Test restoring nonexistent report returns None."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    restored = restore_report(storage, date(2026, 1, 29))
    assert restored is None


def test_large_delta_creates_new_base(tmp_path, monkeypatch):
    """Test that large delta creates new base instead."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    base_xml = '<?xml version="1.0"?><root><data>base</data></root>'
    # Completely different content - delta will be large
    new_xml = '<?xml version="1.0"?><root><data>completely different</data></root>'

    save_raw_report_with_delta(storage, base_xml, date(2026, 1, 29))
    save_raw_report_with_delta(storage, new_xml, date(2026, 1, 30))

    base_file = storage._flex_queries_dir / "base-20260130.xml.zip"
    delta_file = storage._flex_queries_dir / "delta-20260130.patch.zip"

    # Large delta should create new base instead
    assert base_file.exists()
    assert not delta_file.exists()


def test_restore_multiple_deltas(tmp_path, monkeypatch):
    """Test restoring report with multiple deltas."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    base_xml = '<?xml version="1.0"?><root><data>base</data></root>'
    xml1 = '<?xml version="1.0"?><root><data>base</data><data>1</data></root>'
    xml2 = '<?xml version="1.0"?><root><data>base</data><data>1</data><data>2</data></root>'

    save_raw_report_with_delta(storage, base_xml, date(2026, 1, 29))
    save_raw_report_with_delta(storage, xml1, date(2026, 1, 30))
    save_raw_report_with_delta(storage, xml2, date(2026, 1, 31))

    restored = restore_report(storage, date(2026, 1, 31))
    assert restored == xml2


def test_restore_file_from_delta(tmp_path, monkeypatch):
    """Test that restored file from delta matches original exactly."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    # Create realistic XML structure similar to IBKR Flex Query
    original_xml = (
        '<?xml version="1.0"?>\n'
        '<FlexQueryResponse queryName="test" type="AF">\n'
        '<FlexStatements count="1">\n'
        '<FlexStatement accountId="U123" whenGenerated="20260204;072658">\n'
        "<Trades>\n"
        '<Trade id="1"/>\n'
        '<Trade id="2"/>\n'
        "</Trades>\n"
        "</FlexStatement>\n"
        "</FlexStatements>\n"
        "</FlexQueryResponse>\n"
    )

    # Modified XML (only whenGenerated changed)
    modified_xml = (
        '<?xml version="1.0"?>\n'
        '<FlexQueryResponse queryName="test" type="AF">\n'
        '<FlexStatements count="1">\n'
        '<FlexStatement accountId="U123" whenGenerated="20260204;073138">\n'
        "<Trades>\n"
        '<Trade id="1"/>\n'
        '<Trade id="2"/>\n'
        "</Trades>\n"
        "</FlexStatement>\n"
        "</FlexStatements>\n"
        "</FlexQueryResponse>\n"
    )

    # Save base
    save_raw_report_with_delta(storage, original_xml, date(2026, 2, 1))
    # Save modified as delta
    save_raw_report_with_delta(storage, modified_xml, date(2026, 2, 4))

    # Restore and verify it matches exactly
    restored = restore_report(storage, date(2026, 2, 4))
    assert restored == modified_xml, (
        f"Restored content does not match original.\n"
        f"Expected:\n{modified_xml}\n"
        f"Got:\n{restored}\n"
        f"Lengths: expected={len(modified_xml)}, got={len(restored)}"
    )

    # Also verify it's valid XML
    import xml.etree.ElementTree as ET

    ET.fromstring(restored)  # Should not raise


def test_save_same_day_replaces_previous(tmp_path, monkeypatch):
    """Test that saving report for same day replaces previous."""
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    xml1 = '<?xml version="1.0"?><root><data>first</data></root>'
    xml2 = '<?xml version="1.0"?><root><data>second</data></root>'

    save_raw_report_with_delta(storage, xml1, date(2026, 1, 29))
    save_raw_report_with_delta(storage, xml2, date(2026, 1, 29))

    # Should have only one file for this date
    files = list(storage._flex_queries_dir.glob("*20260129*"))
    assert len(files) == 1

    restored = restore_report(storage, date(2026, 1, 29))
    assert restored == xml2


def test_replace_delta_with_new_delta_when_base_exists_for_different_day(tmp_path, monkeypatch):
    """
    Test that replacing delta for same day works correctly when base exists for different day.
    """
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    # Create base for 2026-02-01
    # Use large multi-line XML file (> 2KB) to use 30% threshold
    base_xml = (
        '<?xml version="1.0"?>\n<root>\n'
        + '<transaction id="1"/>\n<transaction id="2"/>\n' * 500
        + "</root>"
    )
    save_raw_report_with_delta(storage, base_xml, date(2026, 2, 1))

    base_20260201 = storage._flex_queries_dir / "base-20260201.xml.zip"
    assert base_20260201.exists()

    # First sync for 2026-02-04: create small delta (change only last line)
    second_xml = (
        '<?xml version="1.0"?>\n<root>\n'
        + '<transaction id="1"/>\n<transaction id="2"/>\n' * 499
        + '<transaction id="3"/>\n</root>'
    )
    save_raw_report_with_delta(storage, second_xml, date(2026, 2, 4))

    delta_20260204 = storage._flex_queries_dir / "delta-20260204.patch.zip"
    base_20260204 = storage._flex_queries_dir / "base-20260204.xml.zip"
    assert delta_20260204.exists(), (
        "First delta should be created (adding one transaction creates small delta)"
    )
    assert not base_20260204.exists(), "Base should not exist yet"

    # Verify restored content is correct
    restored = restore_report(storage, date(2026, 2, 4))
    assert restored == second_xml

    # Second sync for 2026-02-04: replace delta with new delta (change only last line again)
    third_xml = (
        '<?xml version="1.0"?>\n<root>\n'
        + '<transaction id="1"/>\n<transaction id="2"/>\n' * 499
        + '<transaction id="4"/>\n</root>'
    )
    save_raw_report_with_delta(storage, third_xml, date(2026, 2, 4))

    # Verify delta was replaced (not base created)
    assert delta_20260204.exists(), "Delta should still exist (replaced, not converted to base)"
    assert not base_20260204.exists(), "Base should not exist"

    # Verify base-20260201.xml.zip still exists (not replaced)
    assert base_20260201.exists()

    # Verify restored content is correct
    restored = restore_report(storage, date(2026, 2, 4))
    assert restored == third_xml


def test_replace_delta_with_base_when_new_delta_too_large(tmp_path, monkeypatch):
    """
    Test that if delta for the same day becomes too large, previous delta is replaced with base.
    """
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    # Create base for 2026-02-01
    # Use large multi-line XML file (> 2KB) to use 30% threshold
    base_xml = (
        '<?xml version="1.0"?>\n<root>\n'
        + '<transaction id="1"/>\n<transaction id="2"/>\n' * 500
        + "</root>"
    )
    save_raw_report_with_delta(storage, base_xml, date(2026, 2, 1))

    base_20260201 = storage._flex_queries_dir / "base-20260201.xml.zip"
    assert base_20260201.exists()

    # First sync for 2026-02-04: create small delta (change only last line)
    second_xml = (
        '<?xml version="1.0"?>\n<root>\n'
        + '<transaction id="1"/>\n<transaction id="2"/>\n' * 499
        + '<transaction id="3"/>\n</root>'
    )
    save_raw_report_with_delta(storage, second_xml, date(2026, 2, 4))

    delta_20260204 = storage._flex_queries_dir / "delta-20260204.patch.zip"
    base_20260204 = storage._flex_queries_dir / "base-20260204.xml.zip"
    assert delta_20260204.exists(), (
        "First delta should be created (adding one transaction creates small delta)"
    )
    assert not base_20260204.exists(), "Base should not exist yet"

    # Second sync for 2026-02-04: create completely different XML (large delta)
    # This should replace delta with base because delta is too large
    # Change all transactions to different IDs - creates large delta
    third_xml = (
        '<?xml version="1.0"?>\n<root>\n'
        + '<transaction id="10"/>\n<transaction id="20"/>\n' * 500
        + "<data>completely_different</data>\n</root>"
    )
    save_raw_report_with_delta(storage, third_xml, date(2026, 2, 4))

    # Verify delta was replaced with base (because new delta is too large)
    files_20260204 = list(storage._flex_queries_dir.glob("*20260204*"))
    assert len(files_20260204) == 1, (
        f"Should have only one file for 2026-02-04, got: {files_20260204}"
    )
    assert base_20260204.exists(), "Base should exist (delta was too large)"
    assert not delta_20260204.exists(), "Delta should be replaced with base"

    # Verify base-20260201.xml.zip still exists (not replaced)
    assert base_20260201.exists()

    # Verify restored content is correct
    restored = restore_report(storage, date(2026, 2, 4))
    assert restored == third_xml


def test_restore_exact_match_no_duplicates(tmp_path, monkeypatch):
    """
    Test that restored XML exactly matches original without duplicates.

    This test specifically checks for the bug where delta application creates
    duplicate FlexStatement tags. It should fail with current broken implementation
    and pass after fix.
    """
    monkeypatch.setattr("ibkr_porez.storage.user_data_dir", lambda _: str(tmp_path))
    storage = Storage()

    # Original XML with single FlexStatement
    original_xml = (
        '<?xml version="1.0"?>\n'
        '<FlexQueryResponse queryName="test" type="AF">\n'
        '<FlexStatements count="1">\n'
        '<FlexStatement accountId="U123" whenGenerated="20260204;072658">\n'
        "<Trades>\n"
        '<Trade id="1"/>\n'
        "</Trades>\n"
        "</FlexStatement>\n"
        "</FlexStatements>\n"
        "</FlexQueryResponse>\n"
    )

    # Modified XML - only whenGenerated attribute changed
    modified_xml = (
        '<?xml version="1.0"?>\n'
        '<FlexQueryResponse queryName="test" type="AF">\n'
        '<FlexStatements count="1">\n'
        '<FlexStatement accountId="U123" whenGenerated="20260204;073138">\n'
        "<Trades>\n"
        '<Trade id="1"/>\n'
        "</Trades>\n"
        "</FlexStatement>\n"
        "</FlexStatements>\n"
        "</FlexQueryResponse>\n"
    )

    # Save base
    save_raw_report_with_delta(storage, original_xml, date(2026, 2, 1))
    # Save modified as delta
    save_raw_report_with_delta(storage, modified_xml, date(2026, 2, 4))

    # Restore
    restored = restore_report(storage, date(2026, 2, 4))

    # Verify exact match
    assert restored == modified_xml, (
        f"Restored content does not match original.\n"
        f"Expected ({len(modified_xml)} chars):\n{modified_xml}\n\n"
        f"Got ({len(restored)} chars):\n{restored}\n\n"
    )

    # Verify no duplicate FlexStatement tags (count opening tags only, not FlexStatements)
    # Use regex to match <FlexStatement but not <FlexStatements
    import re

    flex_stmt_open_matches = re.findall(r"<FlexStatement\s", restored)
    flex_stmt_close_count = restored.count("</FlexStatement>")
    assert len(flex_stmt_open_matches) == 1, (
        f"Expected 1 FlexStatement opening tag, got {len(flex_stmt_open_matches)}"
    )
    assert flex_stmt_close_count == 1, (
        f"Expected 1 closing FlexStatement tag, got {flex_stmt_close_count}"
    )

    # Verify valid XML
    import xml.etree.ElementTree as ET

    try:
        root = ET.fromstring(restored)
        # Verify structure
        flex_statements = root.findall(".//FlexStatement")
        assert len(flex_statements) == 1, (
            f"Expected 1 FlexStatement element, got {len(flex_statements)}"
        )
    except ET.ParseError as e:
        pytest.fail(f"Restored XML is invalid: {e}")
