use ibkr_porez::models::*;
use pretty_assertions::assert_eq;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/resources/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).unwrap()
}

// ---------------------------------------------------------------------------
// Enum roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_transaction_type_serde() {
    let v = TransactionType::WithholdingTax;
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, r#""WITHHOLDING_TAX""#);
    let back: TransactionType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, TransactionType::WithholdingTax);
}

#[test]
fn test_declaration_type_serde() {
    let json = serde_json::to_string(&DeclarationType::Ppdg3r).unwrap();
    assert_eq!(json, r#""PPDG-3R""#);

    let json = serde_json::to_string(&DeclarationType::Ppo).unwrap();
    assert_eq!(json, r#""PP OPO""#);

    let back: DeclarationType = serde_json::from_str(r#""PPDG-3R""#).unwrap();
    assert_eq!(back, DeclarationType::Ppdg3r);

    let back: DeclarationType = serde_json::from_str(r#""PP OPO""#).unwrap();
    assert_eq!(back, DeclarationType::Ppo);
}

#[test]
fn test_declaration_status_lowercase() {
    let json = serde_json::to_string(&DeclarationStatus::Draft).unwrap();
    assert_eq!(json, r#""draft""#);

    let back: DeclarationStatus = serde_json::from_str(r#""submitted""#).unwrap();
    assert_eq!(back, DeclarationStatus::Submitted);
}

// ---------------------------------------------------------------------------
// Transaction fixture deserialization (real obfuscated data)
// ---------------------------------------------------------------------------

#[test]
fn test_read_transactions_fixture() {
    let content = read_fixture("transactions.json");
    let txns: Vec<Transaction> = serde_json::from_str(&content).unwrap();
    assert_eq!(txns.len(), 79);

    let t0 = &txns[0];
    assert_eq!(t0.transaction_id, "8731160515");
    assert_eq!(t0.symbol, "AAAA");
    assert_eq!(t0.r#type, TransactionType::Trade);
    assert_eq!(
        t0.date,
        chrono::NaiveDate::from_ymd_opt(2025, 12, 23).unwrap()
    );
    assert!(t0.open_date.is_none());
    assert_eq!(t0.open_price, Some(rust_decimal::Decimal::ZERO));

    // DIVIDEND record
    let dividends: Vec<_> = txns.iter().filter(|t| t.r#type == TransactionType::Dividend).collect();
    assert!(!dividends.is_empty());
    assert!(dividends[0].open_price.is_none());

    // WITHHOLDING_TAX record
    let wht: Vec<_> = txns
        .iter()
        .filter(|t| t.r#type == TransactionType::WithholdingTax)
        .collect();
    assert!(!wht.is_empty());
    assert!(wht[0].open_price.is_none());

    // All-null columns (open_date, exchange_rate, amount_rsd) should be missing from JSON
    // but deserialized as None thanks to #[serde(default)]
    for t in &txns {
        assert!(t.open_date.is_none());
        assert!(t.exchange_rate.is_none());
        assert!(t.amount_rsd.is_none());
    }
}

// ---------------------------------------------------------------------------
// Config fixture deserialization
// ---------------------------------------------------------------------------

#[test]
fn test_read_config_fixture() {
    let content = read_fixture("config.json");
    let cfg: UserConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(cfg.full_name, "Test User");
    assert_eq!(cfg.ibkr_token, "fake-token-12345");
    assert_eq!(cfg.city_code, "223");
    assert_eq!(cfg.data_dir.as_deref(), Some("/tmp/test-data"));
    assert!(cfg.output_folder.is_none());
}

#[test]
fn test_config_defaults_on_empty() {
    let cfg: UserConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(cfg.city_code, "223");
    assert_eq!(cfg.phone, "0600000000");
    assert_eq!(cfg.email, "email@example.com");
}

#[test]
fn test_config_defaults_on_invalid_json() {
    let result: Result<UserConfig, _> = serde_json::from_str("not json at all");
    assert!(result.is_err(), "invalid JSON should fail to parse");
    let cfg = result.unwrap_or_default();
    assert_eq!(cfg.city_code, "223");
    assert_eq!(cfg.full_name, "");
}

// ---------------------------------------------------------------------------
// Rates fixture deserialization
// ---------------------------------------------------------------------------

#[test]
fn test_read_rates_fixture() {
    let content = read_fixture("rates.json");
    let rates: indexmap::IndexMap<String, String> = serde_json::from_str(&content).unwrap();
    assert_eq!(rates.len(), 28);
    assert_eq!(rates["2025-07-01_USD"], "99.3846");
    // Verify insertion order preserved
    let keys: Vec<&String> = rates.keys().collect();
    assert_eq!(keys[0], "2025-07-01_USD");
}

// ---------------------------------------------------------------------------
// Declarations fixture deserialization
// ---------------------------------------------------------------------------

#[test]
fn test_read_declarations_fixture() {
    let content = read_fixture("declarations.json");
    let file: DeclarationsFile = serde_json::from_str(&content).unwrap();
    assert_eq!(file.declarations.len(), 6);
    assert_eq!(file.last_declaration_date.as_deref(), Some("2026-03-10"));

    let d1: Declaration = serde_json::from_value(file.declarations[0].clone()).unwrap();
    assert_eq!(d1.declaration_id, "1");
    assert_eq!(d1.r#type, DeclarationType::Ppdg3r);
    assert_eq!(d1.status, DeclarationStatus::Pending);
    assert!(d1.submitted_at.is_some());
    assert!(d1.report_data.is_some());
    assert_eq!(d1.report_data.as_ref().unwrap().len(), 14);

    let entry: TaxReportEntry =
        serde_json::from_value(d1.report_data.unwrap()[0].clone()).unwrap();
    assert_eq!(entry.ticker, "CCC");
    assert!(!entry.is_tax_exempt);

    let d2: Declaration = serde_json::from_value(file.declarations[1].clone()).unwrap();
    assert_eq!(d2.declaration_id, "2");
    assert_eq!(d2.r#type, DeclarationType::Ppo);
    assert_eq!(d2.status, DeclarationStatus::Finalized);
    assert!(d2.submitted_at.is_some());

    let ie: IncomeDeclarationEntry =
        serde_json::from_value(d2.report_data.unwrap()[0].clone()).unwrap();
    assert_eq!(ie.sifra_vrste_prihoda, INCOME_CODE_DIVIDEND);
}

// ---------------------------------------------------------------------------
// Transaction key and identity
// ---------------------------------------------------------------------------

#[test]
fn test_transaction_make_key() {
    let t1 = make_test_transaction("TX-001", "ACME", "10", "150.5");
    let t2 = make_test_transaction("TX-002", "ACME", "10", "150.5");
    assert_eq!(t1.make_key(), t2.make_key());

    let t3 = make_test_transaction("TX-003", "ACME", "10", "150.6");
    assert_ne!(t1.make_key(), t3.make_key());
}

#[test]
fn test_is_identical_record() {
    let t1 = make_test_transaction("TX-001", "ACME", "10", "150.5");
    let t2 = make_test_transaction("TX-001", "ACME", "10", "150.5");
    assert!(t1.is_identical_to(&t2));

    let t3 = make_test_transaction("TX-001", "ACME", "11", "150.5");
    assert!(!t1.is_identical_to(&t3));
}

fn make_test_transaction(id: &str, symbol: &str, qty: &str, price: &str) -> Transaction {
    use rust_decimal::Decimal;
    use std::str::FromStr;
    Transaction {
        transaction_id: id.into(),
        date: chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap(),
        r#type: TransactionType::Trade,
        symbol: symbol.into(),
        description: format!("Buy {symbol}"),
        quantity: Decimal::from_str(qty).unwrap(),
        price: Decimal::from_str(price).unwrap(),
        amount: Decimal::from_str("-1505").unwrap(),
        currency: Currency::USD,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
    }
}
