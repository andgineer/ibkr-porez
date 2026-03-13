use chrono::NaiveDate;
use ibkr_porez::declaration_manager::DeclarationManager;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::{
    Currency, Declaration, DeclarationStatus, DeclarationType, Transaction, TransactionType,
    UserConfig,
};
use ibkr_porez::nbs::NBSClient;
use ibkr_porez::report_gains::generate_gains_report;
use ibkr_porez::report_income::generate_income_reports;
use ibkr_porez::storage::Storage;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn test_config() -> UserConfig {
    UserConfig {
        ibkr_token: "tok".into(),
        ibkr_query_id: "qid".into(),
        personal_id: "1234567890123".into(),
        full_name: "Test User".into(),
        address: "Test Street 1".into(),
        city_code: "223".into(),
        phone: "0641234567".into(),
        email: "test@test.com".into(),
        data_dir: None,
        output_folder: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn make_txn(
    id: &str,
    txn_type: TransactionType,
    symbol: &str,
    date: &str,
    quantity: Decimal,
    price: Decimal,
    amount: Decimal,
    currency: Currency,
    description: &str,
) -> Transaction {
    Transaction {
        transaction_id: id.to_string(),
        date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        r#type: txn_type,
        symbol: symbol.to_string(),
        description: description.to_string(),
        quantity,
        price,
        amount,
        currency,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
    }
}

fn setup_with_rates(rates: &[(&str, &str, &str)]) -> (tempfile::TempDir, Storage, HolidayCalendar) {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    let mut rate_map = indexmap::IndexMap::new();
    for (date, currency, rate) in rates {
        let key = format!("{date}_{currency}");
        rate_map.insert(key, rate.to_string());
    }
    storage.write_rates(&rate_map).unwrap();

    let mut cal = HolidayCalendar::empty();
    cal.set_fallback(true);

    (tmp, storage, cal)
}

#[test]
fn test_gains_report_known_trades() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-01-15", "USD", "117.50"),
        ("2023-03-10", "USD", "117.00"),
        ("2023-06-15", "USD", "108.00"),
    ]);

    let txns = vec![
        make_txn(
            "t1",
            TransactionType::Trade,
            "AAPL",
            "2023-01-15",
            dec!(10),
            dec!(150),
            dec!(1500),
            Currency::USD,
            "",
        ),
        make_txn(
            "t2",
            TransactionType::Trade,
            "MSFT",
            "2023-03-10",
            dec!(5),
            dec!(200),
            dec!(1000),
            Currency::USD,
            "",
        ),
        make_txn(
            "t3",
            TransactionType::Trade,
            "AAPL",
            "2023-06-15",
            dec!(-10),
            dec!(170),
            dec!(-1700),
            Currency::USD,
            "",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let period_start = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
    let period_end = NaiveDate::from_ymd_opt(2023, 6, 30).unwrap();

    let report = generate_gains_report(
        &storage,
        &nbs,
        &test_config(),
        &cal,
        period_start,
        period_end,
        false,
    )
    .unwrap();

    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.entries[0].ticker, "AAPL");
    assert_eq!(report.entries[0].quantity, dec!(10));
    assert!(report.filename.contains("ppdg3r"));
    assert!(report.filename.contains("H1"));
    assert!(report.xml_content.contains("xmlns:ns1"));
}

#[test]
fn test_gains_report_empty_period_returns_error() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2023-01-15", "USD", "117.50")]);

    let txns = vec![make_txn(
        "t1",
        TransactionType::Trade,
        "AAPL",
        "2023-01-15",
        dec!(10),
        dec!(150),
        dec!(1500),
        Currency::USD,
        "",
    )];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let period_start = NaiveDate::from_ymd_opt(2023, 7, 1).unwrap();
    let period_end = NaiveDate::from_ymd_opt(2023, 12, 31).unwrap();

    let result = generate_gains_report(
        &storage,
        &nbs,
        &test_config(),
        &cal,
        period_start,
        period_end,
        false,
    );
    let err = result.err().expect("should fail with no taxable sales");
    assert!(err.to_string().contains("no taxable sales"));
}

#[test]
fn test_gains_report_metadata() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-01-15", "USD", "100.00"),
        ("2023-06-15", "USD", "100.00"),
    ]);

    let txns = vec![
        make_txn(
            "t1",
            TransactionType::Trade,
            "X",
            "2023-01-15",
            dec!(10),
            dec!(100),
            dec!(1000),
            Currency::USD,
            "",
        ),
        make_txn(
            "t2",
            TransactionType::Trade,
            "X",
            "2023-06-15",
            dec!(-10),
            dec!(120),
            dec!(-1200),
            Currency::USD,
            "",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let period_start = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
    let period_end = NaiveDate::from_ymd_opt(2023, 6, 30).unwrap();

    let report = generate_gains_report(
        &storage,
        &nbs,
        &test_config(),
        &cal,
        period_start,
        period_end,
        false,
    )
    .unwrap();
    let meta = report.metadata();

    assert_eq!(meta["entry_count"], 1);
    assert_eq!(meta["period_start"], "2023-01-01");
    assert_eq!(meta["period_end"], "2023-06-30");
}

#[test]
fn test_income_reports_dividend_grouping() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-07-15", "USD", "108.00"),
        ("2023-07-17", "USD", "108.00"),
    ]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(50.0),
            Currency::USD,
            "VOO.US(US9229083632) Cash Dividend",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2023-07-17",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-7.50),
            Currency::USD,
            "VOO.US(US9229083632) Tax",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2023, 7, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2023, 7, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();

    assert_eq!(reports.len(), 1);
    assert!(reports[0].filename.contains("ppopo"));
    assert!(reports[0].filename.contains("voo"));
    assert!(reports[0].xml_content.contains("xmlns:ns1"));
}

#[test]
fn test_income_reports_coupon_groups_by_currency() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-07-15", "USD", "108.00"),
        ("2023-07-15", "EUR", "117.00"),
        ("2023-07-17", "USD", "108.00"),
        ("2023-07-17", "EUR", "117.00"),
    ]);

    let txns = vec![
        make_txn(
            "i1",
            TransactionType::Interest,
            "BOND-USD",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "Bond interest USD",
        ),
        make_txn(
            "i2",
            TransactionType::Interest,
            "BOND-EUR",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(80.0),
            Currency::EUR,
            "Bond interest EUR",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "BOND-USD",
            "2023-07-17",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "Bond WHT USD",
        ),
        make_txn(
            "w2",
            TransactionType::WithholdingTax,
            "BOND-EUR",
            "2023-07-17",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-12.0),
            Currency::EUR,
            "Bond WHT EUR",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2023, 7, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2023, 7, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();

    assert_eq!(reports.len(), 2, "should group coupons by currency");
    let filenames: Vec<&str> = reports.iter().map(|r| r.filename.as_str()).collect();
    assert!(filenames.iter().any(|f| f.contains("usd")));
    assert!(filenames.iter().any(|f| f.contains("eur")));
}

#[test]
fn test_income_report_metadata() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-07-15", "USD", "108.00"),
        ("2023-07-17", "USD", "108.00"),
    ]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "VOO.US(US9229083632) Cash Dividend",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2023-07-17",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "VOO.US(US9229083632) Tax",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2023, 7, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2023, 7, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    assert!(!reports.is_empty());

    let meta = reports[0].metadata();
    assert_eq!(meta["income_type"], "dividend");
    assert_eq!(meta["symbol"], "VOO");
}

// ---------------------------------------------------------------------------
// WHT matching tests – ported from Python test_withholding_tax_matching.py
// ---------------------------------------------------------------------------

// Python: test_find_tax_in_subsequent_days – WHT 2 days after income, rate=100
#[test]
fn test_wht_found_within_7_day_window() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2025-12-24", "USD", "100.00"),
        ("2025-12-26", "USD", "100.00"),
    ]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "VOO(US9229083632) CASH DIVIDEND",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2025-12-26",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "VOO(US9229083632) US TAX",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    assert_eq!(reports.len(), 1);
    // WHT 15 * 100 = 1500 RSD
    assert_eq!(
        reports[0].entries[0].porez_placen_drugoj_drzavi,
        dec!(1500.00)
    );
}

// Python: test_find_tax_not_found_beyond_range – WHT 9 days after income
#[test]
fn test_wht_not_found_beyond_7_day_window() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2025-12-24", "USD", "100.00"),
        ("2026-01-02", "USD", "100.00"),
    ]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "VOO(US9229083632) CASH DIVIDEND",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2026-01-02",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "VOO(US9229083632) US TAX",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    assert_eq!(reports.len(), 1);
    // WHT beyond window → zero
    assert_eq!(reports[0].entries[0].porez_placen_drugoj_drzavi, dec!(0));
}

// Python: test_match_dividend_by_entity_name_isin – different symbol, same ISIN
#[test]
fn test_wht_matched_by_entity_isin_not_symbol() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2025-12-24", "USD", "100.00")]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "VOO (US9229083632) CASH DIVIDEND",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "DIFFERENT",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "VOO (US9229083632) US TAX",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].entries[0].porez_placen_drugoj_drzavi,
        dec!(1500.00)
    );
}

// Python: test_match_dividend_fallback_to_symbol – no ISIN, matches by symbol
#[test]
fn test_wht_fallback_to_symbol_match() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2025-12-24", "USD", "100.00")]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "VOO CASH DIVIDEND",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "VOO US TAX",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].entries[0].porez_placen_drugoj_drzavi,
        dec!(1500.00)
    );
}

// Python: test_match_interest_by_currency – coupons match by currency
#[test]
fn test_wht_interest_matched_by_currency() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2025-12-24", "USD", "100.00")]);

    let txns = vec![
        make_txn(
            "i1",
            TransactionType::Interest,
            "",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "USD Credit Interest for Account",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "DIFFERENT",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "USD Interest Tax",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].entries[0].porez_placen_drugoj_drzavi,
        dec!(1500.00)
    );
}

// Python: test_multiple_taxes_summed
#[test]
fn test_wht_multiple_taxes_summed() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2025-12-24", "USD", "100.00"),
        ("2025-12-25", "USD", "100.00"),
    ]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "VOO CASH DIVIDEND",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-10.0),
            Currency::USD,
            "VOO US TAX 1",
        ),
        make_txn(
            "w2",
            TransactionType::WithholdingTax,
            "VOO",
            "2025-12-25",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-5.0),
            Currency::USD,
            "VOO US TAX 2",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    assert_eq!(reports.len(), 1);
    // (10 + 5) * 100 = 1500 RSD
    assert_eq!(
        reports[0].entries[0].porez_placen_drugoj_drzavi,
        dec!(1500.00)
    );
}

// Python: zero WHT + force=false → error
#[test]
fn test_zero_wht_force_false_errors() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2025-12-24", "USD", "100.00")]);

    let txns = vec![make_txn(
        "d1",
        TransactionType::Dividend,
        "VOO",
        "2025-12-24",
        Decimal::ZERO,
        Decimal::ZERO,
        dec!(100.0),
        Currency::USD,
        "VOO CASH DIVIDEND",
    )];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let result = generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, false);
    let err = result
        .err()
        .expect("should error on zero WHT without force");
    assert!(err.to_string().contains("withholding tax"));
}

// Python: interest grouping by currency – two symbols same currency → one report
#[test]
fn test_interest_grouped_by_currency_not_symbol() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2025-12-24", "USD", "100.00")]);

    let txns = vec![
        make_txn(
            "i1",
            TransactionType::Interest,
            "",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "USD Credit Interest",
        ),
        make_txn(
            "i2",
            TransactionType::Interest,
            "CASH",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(50.0),
            Currency::USD,
            "USD Debit Interest",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "USD Interest Tax",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    // Both interest entries share USD currency → one declaration
    assert_eq!(reports.len(), 1);
    // total bruto = (100 + 50) * 100 = 15000 RSD
    assert_eq!(reports[0].entries[0].bruto_prihod, dec!(15000.00));
}

// Python: test_xml_generator_tax_calculation – verify obracunati_porez = bruto * 0.15
#[test]
fn test_income_tax_calculation_matches_python() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2025-12-24", "USD", "99.45")]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(21.25),
            Currency::USD,
            "VOO(US9229083632) CASH DIVIDEND",
        ),
        make_txn(
            "d2",
            TransactionType::Dividend,
            "SGOV",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(87.55),
            Currency::USD,
            "SGOV(US46436E7186) CASH DIVIDEND",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-5.0),
            Currency::USD,
            "VOO(US9229083632) TAX",
        ),
        make_txn(
            "w2",
            TransactionType::WithholdingTax,
            "SGOV",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-5.0),
            Currency::USD,
            "SGOV(US46436E7186) TAX",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = NBSClient::new(&storage, &cal);
    let start = NaiveDate::from_ymd_opt(2025, 12, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    let reports =
        generate_income_reports(&storage, &nbs, &test_config(), &cal, start, end, true).unwrap();
    // VOO and SGOV are different symbols, same date → 2 separate reports
    assert_eq!(reports.len(), 2);

    for r in &reports {
        let e = &r.entries[0];
        // obracunati = bruto * 0.15 ROUND_HALF_UP
        let expected_tax = (e.bruto_prihod * dec!(0.15))
            .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
        assert_eq!(e.obracunati_porez, expected_tax);
        assert_eq!(e.osnovica_za_porez, e.bruto_prihod);
    }
}

// ---------------------------------------------------------------------------
// Declaration lifecycle tests – ported from Python test_sync.py
// ---------------------------------------------------------------------------

#[test]
fn test_declaration_lifecycle_submit_preserves_on_resync() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    let decl = Declaration {
        declaration_id: "1".into(),
        r#type: DeclarationType::Ppo,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2025, 12, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        created_at: chrono::Local::now().naive_local(),
        submitted_at: None,
        paid_at: None,
        file_path: Some("/tmp/001-ppopo-voo-2025-1224.xml".into()),
        xml_content: Some("<xml>test</xml>".into()),
        report_data: None,
        metadata: indexmap::IndexMap::new(),
        attached_files: indexmap::IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();

    let mgr = DeclarationManager::new(&storage);
    mgr.submit(&["1"]).unwrap();

    // Default tax_due_rsd is 1 (positive) → PP-OPO goes to Submitted
    let submitted = storage.get_declarations(None, None);
    assert_eq!(submitted.len(), 1);
    assert_eq!(submitted[0].status, DeclarationStatus::Submitted);
    assert!(submitted[0].submitted_at.is_some());

    // Adding a new declaration doesn't affect the existing submitted one
    let decl2 = Declaration {
        declaration_id: "2".into(),
        r#type: DeclarationType::Ppo,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
        created_at: chrono::Local::now().naive_local(),
        submitted_at: None,
        paid_at: None,
        file_path: Some("/tmp/002-ppopo-voo-2026-0115.xml".into()),
        xml_content: Some("<xml>test2</xml>".into()),
        report_data: None,
        metadata: indexmap::IndexMap::new(),
        attached_files: indexmap::IndexMap::new(),
    };
    storage.save_declaration(&decl2).unwrap();

    let all = storage.get_declarations(None, None);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].status, DeclarationStatus::Submitted);
    assert_eq!(all[1].status, DeclarationStatus::Draft);
}

#[test]
fn test_declaration_pay_marks_finalized() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    let decl = Declaration {
        declaration_id: "1".into(),
        r#type: DeclarationType::Ppdg3r,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        created_at: chrono::Local::now().naive_local(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: Some("<xml>gains</xml>".into()),
        report_data: None,
        metadata: indexmap::IndexMap::new(),
        attached_files: indexmap::IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();

    let mgr = DeclarationManager::new(&storage);
    mgr.submit(&["1"]).unwrap();

    let pending = storage.get_declarations(None, None);
    assert_eq!(pending[0].status, DeclarationStatus::Pending);

    mgr.pay(&["1"]).unwrap();

    let finalized = storage.get_declarations(None, None);
    assert_eq!(finalized[0].status, DeclarationStatus::Finalized);
    assert!(finalized[0].paid_at.is_some());
}
