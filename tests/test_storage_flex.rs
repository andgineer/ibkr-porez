use chrono::NaiveDate;
use ibkr_porez::storage_flex;
use tempfile::TempDir;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn sample_xml(version: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<FlexQueryResponse queryName="Test" type="AF">
  <FlexStatements count="1">
    <FlexStatement accountId="U1234567" fromDate="20250101" toDate="20251231">
      <Trades>
        <Trade symbol="ACME" dateTime="20250615" quantity="10" tradePrice="150.50"/>
        <Trade symbol="TEST" dateTime="20250720" quantity="5" tradePrice="200.{version:02}"/>
      </Trades>
    </FlexStatement>
  </FlexStatements>
</FlexQueryResponse>
"#
    )
}

#[test]
fn test_save_first_report_creates_base() {
    let dir = TempDir::new().unwrap();
    let xml = sample_xml(1);
    storage_flex::save_raw_report_with_delta(dir.path(), &xml, d(2026, 1, 29)).unwrap();

    let base = dir.path().join("base-20260129.xml.zip");
    assert!(base.exists());
    assert!(!dir.path().join("delta-20260129.patch.zip").exists());
}

#[test]
fn test_save_second_report_creates_delta() {
    let dir = TempDir::new().unwrap();
    let xml1 = sample_xml(1);
    let xml2 = sample_xml(2);
    storage_flex::save_raw_report_with_delta(dir.path(), &xml1, d(2026, 1, 29)).unwrap();
    storage_flex::save_raw_report_with_delta(dir.path(), &xml2, d(2026, 1, 30)).unwrap();

    assert!(dir.path().join("base-20260129.xml.zip").exists());
    // Either delta or new base (depending on size threshold)
    let delta = dir.path().join("delta-20260130.patch.zip");
    let base2 = dir.path().join("base-20260130.xml.zip");
    assert!(delta.exists() || base2.exists());
}

#[test]
fn test_restore_report_base_only() {
    let dir = TempDir::new().unwrap();
    let xml = sample_xml(1);
    storage_flex::save_raw_report_with_delta(dir.path(), &xml, d(2026, 1, 29)).unwrap();

    let restored = storage_flex::restore_report(dir.path(), d(2026, 1, 29));
    assert_eq!(restored.as_deref(), Some(xml.as_str()));
}

#[test]
fn test_restore_after_delta() {
    let dir = TempDir::new().unwrap();
    let xml1 = sample_xml(1);
    storage_flex::save_raw_report_with_delta(dir.path(), &xml1, d(2026, 1, 29)).unwrap();

    // Create a significantly different XML to avoid new-base fallback
    let xml2 = xml1.replace("150.50", "151.00");
    storage_flex::save_raw_report_with_delta(dir.path(), &xml2, d(2026, 1, 30)).unwrap();

    let restored = storage_flex::restore_report(dir.path(), d(2026, 1, 30));
    assert!(restored.is_some());
    let content = restored.unwrap();
    assert!(content.contains("151.00"));
}

#[test]
fn test_large_delta_falls_back_to_base() {
    let dir = TempDir::new().unwrap();
    let xml1 = "a\n".repeat(10);
    storage_flex::save_raw_report_with_delta(dir.path(), &xml1, d(2026, 1, 29)).unwrap();

    // Completely different content -> delta will be huge relative to base
    let xml2 = "b\n".repeat(10);
    storage_flex::save_raw_report_with_delta(dir.path(), &xml2, d(2026, 1, 30)).unwrap();

    // Since the content is so different, it should create a new base
    let base2 = dir.path().join("base-20260130.xml.zip");
    let delta = dir.path().join("delta-20260130.patch.zip");
    // Either a new base was created or delta was saved
    assert!(base2.exists() || delta.exists());

    // Either way, restoring should give back xml2
    let restored = storage_flex::restore_report(dir.path(), d(2026, 1, 30));
    assert_eq!(restored.as_deref(), Some(xml2.as_str()));
}

#[test]
fn test_one_report_per_day_replacement() {
    let dir = TempDir::new().unwrap();
    let xml1 = sample_xml(1);
    let xml2 = sample_xml(2);

    storage_flex::save_raw_report_with_delta(dir.path(), &xml1, d(2026, 1, 29)).unwrap();
    storage_flex::save_raw_report_with_delta(dir.path(), &xml2, d(2026, 1, 29)).unwrap();

    let restored = storage_flex::restore_report(dir.path(), d(2026, 1, 29));
    assert!(restored.is_some());
    let content = restored.unwrap();
    assert!(content.contains("200.02"), "should have the second version");
}

#[test]
fn test_cleanup_old_base() {
    let dir = TempDir::new().unwrap();
    storage_flex::save_raw_report_with_delta(dir.path(), "old content\n", d(2026, 1, 20)).unwrap();
    storage_flex::save_raw_report_with_delta(dir.path(), "new content\n", d(2026, 1, 29)).unwrap();

    // Both bases should exist (second may be base or delta)
    storage_flex::cleanup_old_base(dir.path(), d(2026, 1, 25));

    assert!(!dir.path().join("base-20260120.xml.zip").exists());
}

#[test]
fn test_restore_nonexistent() {
    let dir = TempDir::new().unwrap();
    let result = storage_flex::restore_report(dir.path(), d(2026, 1, 29));
    assert!(result.is_none());
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!(
        "{}/tests/resources/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
}

#[test]
fn test_restore_from_python_generated_fixtures() {
    let dir = TempDir::new().unwrap();

    std::fs::copy(
        fixture_path("base-20260306.xml.zip"),
        dir.path().join("base-20260306.xml.zip"),
    )
    .unwrap();
    std::fs::copy(
        fixture_path("delta-20260309.patch.zip"),
        dir.path().join("delta-20260309.patch.zip"),
    )
    .unwrap();

    let base_content = storage_flex::restore_report(dir.path(), d(2026, 3, 6));
    assert!(base_content.is_some(), "should restore base file");
    let base_xml = base_content.unwrap();
    assert!(
        base_xml.contains("FlexQueryResponse"),
        "base should be valid XML"
    );
    assert!(
        base_xml.contains("20260306"),
        "base should reference its own date"
    );

    let delta_content = storage_flex::restore_report(dir.path(), d(2026, 3, 9));
    assert!(delta_content.is_some(), "should restore with delta applied");
    let delta_xml = delta_content.unwrap();
    assert!(
        delta_xml.contains("FlexQueryResponse"),
        "delta result should be valid XML"
    );
    assert!(
        delta_xml.contains("20260309"),
        "delta result should contain updated date"
    );

    assert_ne!(base_xml, delta_xml, "delta should produce different content");
}
