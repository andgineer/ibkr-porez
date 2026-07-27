use chrono::{Datelike, NaiveDate};
use ibkr_porez::declaration_manager::DeclarationManager;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::{
    CarryforwardVintage, Currency, Declaration, DeclarationStatus, DeclarationType, Transaction,
    TransactionType, UserConfig,
};
use ibkr_porez::nbs::NBSClient;
use ibkr_porez::report_gains::{compute_carryforward_application, generate_gains_report};
use ibkr_porez::report_income::{
    Declared, GroupAction, IncomeReport, RenderOptions, SourceAmounts, collect_income_groups,
    decide, render_income_report,
};
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
        action_id: None,
    }
}

/// Unroutable scheme so tests never hit the real NBS API. reqwest rejects it
/// before opening a socket, which a dead port does not: on Windows connecting
/// to one blocks for seconds instead of being refused outright.
const FAKE_NBS_URL: &str = "offline://nbs";

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

fn nbs_offline<'a>(storage: &'a Storage, cal: &'a HolidayCalendar) -> NBSClient<'a> {
    // Single attempt with no delay: the production retry-with-backoff would
    // otherwise add seconds per lookup.
    NBSClient::with_base_url(storage, cal, FAKE_NBS_URL).with_retries(1, std::time::Duration::ZERO)
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

    let nbs = nbs_offline(&storage, &cal);
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

    let nbs = nbs_offline(&storage, &cal);
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

    let nbs = nbs_offline(&storage, &cal);
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

// ---------------------------------------------------------------------------
// compute_carryforward_application
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn make_vintage(
    id: &str,
    origin_period_end: NaiveDate,
    recognized: Decimal,
    remaining: Decimal,
    expiration_tax_year: i32,
    created_at: chrono::NaiveDateTime,
) -> CarryforwardVintage {
    CarryforwardVintage {
        id: id.into(),
        origin_declaration_id: "origin".into(),
        assessment_reference: None,
        origin_period_start: NaiveDate::from_ymd_opt(origin_period_end.year() - 1, 7, 1).unwrap(),
        origin_period_end,
        recognized_loss_rsd: recognized,
        remaining_loss_rsd: remaining,
        created_at,
        expiration_tax_year,
        notes: None,
    }
}

#[test]
fn test_carryforward_oldest_first_consumption() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    // Older vintage (origin 2023) should be consumed before the newer one (origin 2024).
    storage
        .upsert_carryforward_vintage(make_vintage(
            "CF-old",
            NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
            dec!(500),
            dec!(500),
            2028,
            chrono::NaiveDateTime::default(),
        ))
        .unwrap();
    storage
        .upsert_carryforward_vintage(make_vintage(
            "CF-new",
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            dec!(500),
            dec!(500),
            2029,
            chrono::NaiveDateTime::default(),
        ))
        .unwrap();

    let app = compute_carryforward_application(&storage, dec!(700), 2025);

    assert_eq!(app.opening_carryforward_rsd, dec!(1000));
    assert_eq!(app.carryforward_used_rsd, dec!(700));
    assert_eq!(app.closing_carryforward_rsd, dec!(300));
    assert_eq!(app.adjusted_tax_base_rsd, Decimal::ZERO);
    assert_eq!(app.estimated_tax_rsd, Decimal::ZERO);

    assert_eq!(app.sources.len(), 2);
    assert_eq!(app.sources[0].vintage_id, "CF-old");
    assert_eq!(app.sources[0].amount_used, dec!(500));
    assert_eq!(app.sources[1].vintage_id, "CF-new");
    assert_eq!(app.sources[1].amount_used, dec!(200));
}

#[test]
fn test_carryforward_partial_consumption_of_larger_vintage() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    storage
        .upsert_carryforward_vintage(make_vintage(
            "CF-1",
            NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
            dec!(2000),
            dec!(2000),
            2028,
            chrono::NaiveDateTime::default(),
        ))
        .unwrap();

    let app = compute_carryforward_application(&storage, dec!(500), 2025);

    assert_eq!(app.carryforward_used_rsd, dec!(500));
    assert_eq!(app.closing_carryforward_rsd, dec!(1500));
    assert_eq!(app.adjusted_tax_base_rsd, Decimal::ZERO);
    assert_eq!(app.sources.len(), 1);
    assert_eq!(app.sources[0].amount_used, dec!(500));
}

#[test]
fn test_carryforward_expired_and_current_year_vintages_excluded() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    // Expired: expiration_tax_year < current_tax_year
    storage
        .upsert_carryforward_vintage(make_vintage(
            "CF-expired",
            NaiveDate::from_ymd_opt(2018, 12, 31).unwrap(),
            dec!(1000),
            dec!(1000),
            2023,
            chrono::NaiveDateTime::default(),
        ))
        .unwrap();
    // Originated in the current tax year: not yet eligible (Y, not Y+1..=Y+5).
    storage
        .upsert_carryforward_vintage(make_vintage(
            "CF-current-year",
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            dec!(1000),
            dec!(1000),
            2030,
            chrono::NaiveDateTime::default(),
        ))
        .unwrap();

    let app = compute_carryforward_application(&storage, dec!(5000), 2025);

    assert_eq!(app.opening_carryforward_rsd, Decimal::ZERO);
    assert_eq!(app.carryforward_used_rsd, Decimal::ZERO);
    assert_eq!(app.adjusted_tax_base_rsd, dec!(5000));
    assert!(app.sources.is_empty());
}

#[test]
fn test_carryforward_zero_or_negative_base_consumes_nothing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    storage
        .upsert_carryforward_vintage(make_vintage(
            "CF-1",
            NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
            dec!(1000),
            dec!(1000),
            2028,
            chrono::NaiveDateTime::default(),
        ))
        .unwrap();

    let app = compute_carryforward_application(&storage, Decimal::ZERO, 2025);

    assert_eq!(app.opening_carryforward_rsd, dec!(1000));
    assert_eq!(app.carryforward_used_rsd, Decimal::ZERO);
    assert_eq!(app.closing_carryforward_rsd, dec!(1000));
    assert_eq!(app.adjusted_tax_base_rsd, Decimal::ZERO);
    assert!(app.sources.is_empty());

    // Ledger untouched.
    let v = storage.find_carryforward_vintage("CF-1").unwrap();
    assert_eq!(v.remaining_loss_rsd, dec!(1000));
}
// ---------------------------------------------------------------------------
// PP-OPO income reports – only what needs an exchange rate or storage;
// matching and netting are unit-tested in src/report_income.rs.
// ---------------------------------------------------------------------------

fn dividend_desc(symbol: &str, isin: &str, per_share: &str) -> String {
    format!("{symbol}({isin}) CASH DIVIDEND USD {per_share} PER SHARE (Ordinary Dividend)")
}

fn tax_desc(symbol: &str, isin: &str, per_share: &str) -> String {
    format!("{symbol}({isin}) CASH DIVIDEND USD {per_share} PER SHARE - US TAX")
}

fn income_reports(
    storage: &Storage,
    nbs: &NBSClient,
    cal: &HolidayCalendar,
    start: &str,
    end: &str,
    today: &str,
    force: bool,
) -> Vec<IncomeReport> {
    let day = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    let (start, end, today) = (day(start), day(end), day(today));
    let opts = RenderOptions {
        today,
        force_rates: force,
    };

    collect_income_groups(&storage.load_transactions(), start, end)
        .iter()
        .filter(|group| decide(group, Declared::No, start, today) == GroupAction::Create)
        .map(|group| render_income_report(group, None, nbs, &test_config(), cal, &opts).unwrap())
        .collect()
}

#[test]
fn test_income_reports_dividend_grouping() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2023-07-15", "USD", "108.00")]);

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
            &dividend_desc("VOO", "US9229083632", "1.74"),
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
            &tax_desc("VOO", "US9229083632", "1.74"),
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let reports = income_reports(
        &storage,
        &nbs,
        &cal,
        "2023-07-01",
        "2023-07-31",
        "2023-08-15",
        true,
    );

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
    ]);

    let txns = vec![
        make_txn(
            "i1",
            TransactionType::Interest,
            "",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(100.0),
            Currency::USD,
            "USD CREDIT INT FOR JUN-2023",
        ),
        make_txn(
            "i2",
            TransactionType::Interest,
            "",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(80.0),
            Currency::EUR,
            "EUR CREDIT INT FOR JUN-2023",
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            "WITHHOLDING @ 30% ON CREDIT INT FOR JUN-2023",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let reports = income_reports(
        &storage,
        &nbs,
        &cal,
        "2023-07-01",
        "2023-07-31",
        "2023-08-15",
        true,
    );

    assert_eq!(reports.len(), 2, "should group coupons by currency");
    let filenames: Vec<&str> = reports.iter().map(|r| r.filename.as_str()).collect();
    assert!(filenames.iter().any(|f| f.contains("usd")));
    assert!(filenames.iter().any(|f| f.contains("eur")));
}

#[test]
fn test_income_report_metadata() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2023-07-15", "USD", "108.00")]);

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
            &dividend_desc("VOO", "US9229083632", "1.74"),
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-15.0),
            Currency::USD,
            &tax_desc("VOO", "US9229083632", "1.74"),
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let reports = income_reports(
        &storage,
        &nbs,
        &cal,
        "2023-07-01",
        "2023-07-31",
        "2023-08-15",
        true,
    );
    assert!(!reports.is_empty());

    let meta = reports[0].metadata();
    assert_eq!(meta["income_type"], "dividend");
    assert_eq!(meta["symbol"], "VOO");
}

// The whole group is converted at the income date's rate, the tax included:
// a withholding row posted days later carries no rate of its own.
#[test]
fn test_income_credit_converted_at_income_date_rate() {
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
            &dividend_desc("VOO", "US9229083632", "1.771"),
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
            &tax_desc("VOO", "US9229083632", "1.771"),
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let reports = income_reports(
        &storage,
        &nbs,
        &cal,
        "2025-12-01",
        "2025-12-31",
        "2026-01-20",
        false,
    );

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].entries[0].bruto_prihod, dec!(10000.00));
    assert_eq!(
        reports[0].entries[0].porez_placen_drugoj_drzavi,
        dec!(1500.00)
    );
}

// The §871(k) shape of golden-003: withholding taken and given back on the same
// day nets to zero, so the full 15% is due -- not credited twice.
#[test]
fn test_reversed_withholding_declares_zero_credit_and_full_tax() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2025-12-24", "USD", "99.4483")]);

    let txns = vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "SGOV",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(87.55),
            Currency::USD,
            &dividend_desc("SGOV", "US46436E7186", "0.323046"),
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "SGOV",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(-26.27),
            Currency::USD,
            &tax_desc("SGOV", "US46436E7186", "0.323046"),
        ),
        make_txn(
            "w2",
            TransactionType::WithholdingTax,
            "SGOV",
            "2025-12-24",
            Decimal::ZERO,
            Decimal::ZERO,
            dec!(26.27),
            Currency::USD,
            &tax_desc("SGOV", "US46436E7186", "0.323046"),
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let reports = income_reports(
        &storage,
        &nbs,
        &cal,
        "2025-12-01",
        "2025-12-31",
        "2026-01-20",
        false,
    );

    assert_eq!(reports.len(), 1);
    let entry = &reports[0].entries[0];
    assert_eq!(entry.bruto_prihod, dec!(8706.70));
    assert_eq!(entry.porez_placen_drugoj_drzavi, dec!(0.00));
    assert_eq!(entry.porez_za_uplatu, dec!(1306.01));
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn dividend_and_tax(gross: Decimal, tax: Decimal) -> Vec<Transaction> {
    vec![
        make_txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            gross,
            Currency::USD,
            &dividend_desc("VOO", "US9229083632", "1.74"),
        ),
        make_txn(
            "w1",
            TransactionType::WithholdingTax,
            "VOO",
            "2023-07-15",
            Decimal::ZERO,
            Decimal::ZERO,
            tax,
            Currency::USD,
            &tax_desc("VOO", "US9229083632", "1.74"),
        ),
    ]
}

// The user sees dollars at the broker and dinars in `show`; a declaration that
// records only the converted figures cannot be reconciled against either.
#[test]
fn metadata_carries_source_currency_amounts() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2023-07-15", "USD", "108.00")]);
    storage
        .save_transactions(&dividend_and_tax(dec!(100.0), dec!(-15.0)))
        .unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let reports = income_reports(
        &storage,
        &nbs,
        &cal,
        "2023-07-01",
        "2023-07-31",
        "2023-08-15",
        false,
    );
    assert_eq!(reports.len(), 1);

    let meta = reports[0].metadata();
    assert_eq!(meta["gross_income_ccy"], "100.00");
    assert_eq!(meta["foreign_tax_paid_ccy"], "15.00");
    assert_eq!(meta["currency"], "USD");
    assert_eq!(
        meta["exchange_rate"]
            .as_str()
            .unwrap()
            .parse::<Decimal>()
            .unwrap(),
        dec!(108)
    );
    // The RSD figures stay the converted ones, beside them.
    assert_eq!(meta["gross_income_rsd"], "10800.00");
    assert_eq!(meta["foreign_tax_paid_rsd"], "1620.00");

    assert_eq!(
        SourceAmounts::from_metadata(&meta).unwrap(),
        reports[0].source,
        "what to_metadata wrote must read back unchanged"
    );
}

// A moved exchange rate must not look like a change in income.
#[test]
fn amendment_compares_source_currency_not_rsd() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2023-07-15", "USD", "108.00")]);
    storage
        .save_transactions(&dividend_and_tax(dec!(100.0), dec!(-15.0)))
        .unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let (start, end, today) = (day("2023-07-01"), day("2023-07-31"), day("2023-08-15"));
    let opts = RenderOptions {
        today,
        force_rates: false,
    };

    let groups = collect_income_groups(&storage.load_transactions(), start, end);
    assert_eq!(groups.len(), 1);
    let declared_at_108 =
        render_income_report(&groups[0], None, &nbs, &test_config(), &cal, &opts).unwrap();
    let declared = SourceAmounts::from_metadata(&declared_at_108.metadata()).unwrap();

    let mut rates = indexmap::IndexMap::new();
    rates.insert("2023-07-15_USD".to_string(), "120.00".to_string());
    storage.write_rates(&rates).unwrap();

    let groups = collect_income_groups(&storage.load_transactions(), start, end);
    let rendered_at_120 =
        render_income_report(&groups[0], None, &nbs, &test_config(), &cal, &opts).unwrap();
    assert_ne!(
        declared_at_108.metadata()["gross_income_rsd"],
        rendered_at_120.metadata()["gross_income_rsd"],
        "the RSD figures did move"
    );

    assert_eq!(
        decide(&groups[0], Declared::Yes(&declared), start, today),
        GroupAction::Skip
    );
}

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
            "USD CREDIT INT FOR NOV-2025",
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
            "USD CREDIT INT FOR NOV-2025",
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
            "WITHHOLDING @ 30% ON CREDIT INT FOR NOV-2025",
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let reports = income_reports(
        &storage,
        &nbs,
        &cal,
        "2025-12-01",
        "2025-12-31",
        "2026-01-20",
        true,
    );

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
            &dividend_desc("VOO", "US9229083632", "1.771"),
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
            &dividend_desc("SGOV", "US46436E7186", "0.323046"),
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
            &tax_desc("VOO", "US9229083632", "1.771"),
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
            &tax_desc("SGOV", "US46436E7186", "0.323046"),
        ),
    ];
    storage.save_transactions(&txns).unwrap();

    let nbs = nbs_offline(&storage, &cal);
    let reports = income_reports(
        &storage,
        &nbs,
        &cal,
        "2025-12-01",
        "2025-12-31",
        "2026-01-20",
        true,
    );

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
