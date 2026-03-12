use chrono::NaiveDate;
use ibkr_porez::models::*;
use ibkr_porez::storage::{merge_transactions, Storage};
use pretty_assertions::assert_eq;
use rust_decimal::Decimal;
use std::str::FromStr;
use tempfile::TempDir;

fn make_txn(id: &str, symbol: &str, qty: &str, date: NaiveDate, csv: bool) -> Transaction {
    let tid = if csv {
        format!("csv-{id}")
    } else {
        id.to_string()
    };
    Transaction {
        transaction_id: tid,
        date,
        r#type: TransactionType::Trade,
        symbol: symbol.into(),
        description: format!("Trade {symbol}"),
        quantity: Decimal::from_str(qty).unwrap(),
        price: Decimal::from_str("100.0").unwrap(),
        amount: Decimal::from_str("-1000.0").unwrap(),
        currency: Currency::USD,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

// ---------------------------------------------------------------------------
// Merge logic
// ---------------------------------------------------------------------------

#[test]
fn test_merge_new_into_empty() {
    let mut existing = Vec::new();
    let new = vec![make_txn("001", "ACME", "10", d(2025, 6, 15), false)];
    let (inserted, updated) = merge_transactions(&mut existing, &new);
    assert_eq!(inserted, 1);
    assert_eq!(updated, 0);
    assert_eq!(existing.len(), 1);
}

#[test]
fn test_merge_duplicate_id_identical() {
    let t = make_txn("001", "ACME", "10", d(2025, 6, 15), false);
    let mut existing = vec![t.clone()];
    let (inserted, updated) = merge_transactions(&mut existing, &[t]);
    assert_eq!(inserted, 0);
    assert_eq!(updated, 0);
    assert_eq!(existing.len(), 1);
}

#[test]
fn test_merge_duplicate_id_different_content() {
    let t1 = make_txn("001", "ACME", "10", d(2025, 6, 15), false);
    let mut t2 = t1.clone();
    t2.quantity = Decimal::from_str("20").unwrap();
    let mut existing = vec![t1];
    let (inserted, updated) = merge_transactions(&mut existing, &[t2]);
    assert_eq!(inserted, 0);
    assert_eq!(updated, 1);
    assert_eq!(existing.len(), 1);
    assert_eq!(existing[0].quantity, Decimal::from_str("20").unwrap());
}

#[test]
fn test_xml_over_csv_upgrade() {
    let csv = make_txn("001", "ACME", "10", d(2025, 6, 15), true);
    let xml = make_txn("XML-001", "ACME", "10", d(2025, 6, 15), false);
    let mut existing = vec![csv];
    let (inserted, updated) = merge_transactions(&mut existing, std::slice::from_ref(&xml));
    assert_eq!(inserted, 0);
    assert_eq!(updated, 1);
    assert_eq!(existing.len(), 1);
    assert_eq!(existing[0].transaction_id, "XML-001");
}

#[test]
fn test_csv_does_not_replace_xml() {
    let xml = make_txn("XML-001", "ACME", "10", d(2025, 6, 15), false);
    let csv = make_txn("002", "ACME", "10", d(2025, 6, 15), true);
    let mut existing = vec![xml.clone()];
    let (inserted, updated) = merge_transactions(&mut existing, &[csv]);
    assert_eq!(inserted, 0);
    assert_eq!(updated, 0);
    assert_eq!(existing.len(), 1);
    assert_eq!(existing[0].transaction_id, "XML-001");
}

#[test]
fn test_xml_split_orders_both_kept() {
    let xml1 = make_txn("XML-001", "ACME", "10", d(2025, 6, 15), false);
    let xml2 = make_txn("XML-002", "ACME", "10", d(2025, 6, 15), false);
    let mut existing = vec![xml1.clone()];
    let (inserted, updated) = merge_transactions(&mut existing, std::slice::from_ref(&xml2));
    assert_eq!(inserted, 1);
    assert_eq!(updated, 0);
    assert_eq!(existing.len(), 2);
}

#[test]
fn test_csv_skipped_if_date_covered_by_xml() {
    let xml = make_txn("XML-001", "ACME", "10", d(2025, 6, 15), false);
    let csv_new = make_txn("003", "OTHER", "5", d(2025, 6, 15), true);
    let mut existing = vec![xml.clone()];
    let (inserted, updated) = merge_transactions(&mut existing, &[csv_new]);
    assert_eq!(inserted, 0);
    assert_eq!(updated, 0);
    assert_eq!(existing.len(), 1);
}

#[test]
fn test_xml_supremacy_removes_existing_csv() {
    let csv = make_txn("001", "OTHER", "5", d(2025, 6, 15), true);
    let xml = make_txn("XML-001", "ACME", "10", d(2025, 6, 15), false);
    let mut existing = vec![csv];
    let (inserted, updated) = merge_transactions(&mut existing, std::slice::from_ref(&xml));
    assert!(existing.iter().all(|t| !t.is_csv_sourced()));
    assert!(existing.iter().any(|t| t.transaction_id == "XML-001"));
    assert!(inserted + updated > 0);
}

// ---------------------------------------------------------------------------
// Storage file roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_storage_transactions_roundtrip() {
    let dir = TempDir::new().unwrap();
    let storage = Storage::with_dir(dir.path());

    let txns = vec![
        make_txn("001", "ACME", "10", d(2025, 6, 15), false),
        make_txn("002", "TEST", "5", d(2025, 7, 20), false),
    ];
    storage.write_transactions(&txns).unwrap();

    let loaded = storage.load_transactions();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].transaction_id, "001");
    assert_eq!(loaded[1].symbol, "TEST");
}

#[test]
fn test_storage_rates_roundtrip() {
    let dir = TempDir::new().unwrap();
    let storage = Storage::with_dir(dir.path());

    let rate = ExchangeRate {
        date: d(2025, 6, 15),
        currency: Currency::USD,
        rate: Decimal::from_str("117.25").unwrap(),
    };
    storage.save_exchange_rate(&rate).unwrap();

    let loaded = storage.get_exchange_rate(d(2025, 6, 15), &Currency::USD);
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().rate, Decimal::from_str("117.25").unwrap());

    assert!(storage
        .get_exchange_rate(d(2025, 6, 16), &Currency::USD)
        .is_none());
}

#[test]
fn test_storage_declarations_roundtrip() {
    let dir = TempDir::new().unwrap();
    let storage = Storage::with_dir(dir.path());

    let decl = Declaration {
        declaration_id: "DECL-001".into(),
        r#type: DeclarationType::Ppdg3r,
        status: DeclarationStatus::Draft,
        period_start: d(2025, 1, 1),
        period_end: d(2025, 12, 31),
        created_at: chrono::NaiveDateTime::parse_from_str(
            "2025-12-20T10:30:00",
            "%Y-%m-%dT%H:%M:%S",
        )
        .unwrap(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: None,
        report_data: None,
        metadata: indexmap::IndexMap::new(),
        attached_files: indexmap::IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();

    let loaded = storage.get_declaration("DECL-001");
    assert!(loaded.is_some());
    let dd = loaded.unwrap();
    assert_eq!(dd.r#type, DeclarationType::Ppdg3r);
    assert_eq!(dd.status, DeclarationStatus::Draft);

    assert!(!storage.declaration_exists("NONEXISTENT"));
}

#[test]
fn test_storage_last_declaration_date() {
    let dir = TempDir::new().unwrap();
    let storage = Storage::with_dir(dir.path());

    assert!(storage.get_last_declaration_date().is_none());

    storage
        .set_last_declaration_date(d(2025, 12, 31))
        .unwrap();
    assert_eq!(
        storage.get_last_declaration_date(),
        Some(d(2025, 12, 31))
    );
}

#[test]
fn test_save_transactions_merge() {
    let dir = TempDir::new().unwrap();
    let storage = Storage::with_dir(dir.path());

    let t1 = make_txn("001", "ACME", "10", d(2025, 6, 15), false);
    let (ins, upd) = storage.save_transactions(&[t1]).unwrap();
    assert_eq!(ins, 1);
    assert_eq!(upd, 0);

    let t2 = make_txn("002", "TEST", "5", d(2025, 7, 20), false);
    let (ins, upd) = storage.save_transactions(&[t2]).unwrap();
    assert_eq!(ins, 1);
    assert_eq!(upd, 0);

    assert_eq!(storage.load_transactions().len(), 2);
}

// ---------------------------------------------------------------------------
// get_transactions with date filtering
// ---------------------------------------------------------------------------

#[test]
fn test_get_transactions_date_filter() {
    let dir = TempDir::new().unwrap();
    let storage = Storage::with_dir(dir.path());

    let txns = vec![
        make_txn("001", "ACME", "10", d(2025, 6, 15), false),
        make_txn("002", "TEST", "5", d(2025, 7, 20), false),
        make_txn("003", "OTHER", "3", d(2025, 8, 10), false),
    ];
    storage.write_transactions(&txns).unwrap();

    let all = storage.get_transactions(None, None);
    assert_eq!(all.len(), 3);

    let filtered = storage.get_transactions(Some(d(2025, 7, 1)), Some(d(2025, 7, 31)));
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].transaction_id, "002");

    let from_only = storage.get_transactions(Some(d(2025, 7, 1)), None);
    assert_eq!(from_only.len(), 2);

    let to_only = storage.get_transactions(None, Some(d(2025, 7, 20)));
    assert_eq!(to_only.len(), 2);
}

