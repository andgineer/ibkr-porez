use chrono::NaiveDate;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::{Currency, Transaction, TransactionType};
use ibkr_porez::nbs::NBSClient;
use ibkr_porez::storage::Storage;
use ibkr_porez::tax::TaxCalculator;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn make_trade(
    symbol: &str,
    date: &str,
    quantity: Decimal,
    price: Decimal,
    currency: Currency,
) -> Transaction {
    Transaction {
        transaction_id: format!("t-{symbol}-{date}-{quantity}"),
        date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        r#type: TransactionType::Trade,
        symbol: symbol.to_string(),
        description: String::new(),
        quantity,
        price,
        amount: quantity * price,
        currency,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
    }
}

/// Unreachable URL so tests never accidentally hit the real NBS API.
const FAKE_NBS_URL: &str = "http://127.0.0.1:1";

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
    NBSClient::with_base_url(storage, cal, FAKE_NBS_URL)
}

#[test]
fn test_fifo_single_buy_single_sell() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-01-15", "USD", "117.5"),
        ("2023-06-15", "USD", "108.0"),
    ]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![
        make_trade("AAPL", "2023-01-15", dec!(10), dec!(150.0), Currency::USD),
        make_trade("AAPL", "2023-06-15", dec!(-10), dec!(170.0), Currency::USD),
    ];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 1);

    let e = &entries[0];
    assert_eq!(e.ticker, "AAPL");
    assert_eq!(e.quantity, dec!(10));
    assert_eq!(e.sale_date, NaiveDate::from_ymd_opt(2023, 6, 15).unwrap());
    assert_eq!(
        e.purchase_date,
        NaiveDate::from_ymd_opt(2023, 1, 15).unwrap()
    );
    assert_eq!(e.sale_price, dec!(170.0));
    assert_eq!(e.purchase_price, dec!(150.0));
    assert!(!e.is_tax_exempt);
}

#[test]
fn test_fifo_partial_lot_consumption() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-01-10", "USD", "100.0"),
        ("2023-03-01", "USD", "100.0"),
        ("2023-06-01", "USD", "100.0"),
    ]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![
        make_trade("VOO", "2023-01-10", dec!(20), dec!(100.0), Currency::USD),
        make_trade("VOO", "2023-03-01", dec!(-5), dec!(110.0), Currency::USD),
        make_trade("VOO", "2023-06-01", dec!(-8), dec!(120.0), Currency::USD),
    ];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 2);

    // First sell: 5 from the lot of 20
    assert_eq!(entries[0].quantity, dec!(5));
    assert_eq!(
        entries[0].purchase_date,
        NaiveDate::from_ymd_opt(2023, 1, 10).unwrap()
    );
    assert_eq!(entries[0].purchase_price, dec!(100.0));

    // Second sell: 8 from remaining 15 in the lot
    assert_eq!(entries[1].quantity, dec!(8));
    assert_eq!(
        entries[1].purchase_date,
        NaiveDate::from_ymd_opt(2023, 1, 10).unwrap()
    );
    assert_eq!(entries[1].purchase_price, dec!(100.0));
}

#[test]
fn test_fifo_multi_lot_sell() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-01-10", "USD", "100.0"),
        ("2023-02-10", "USD", "100.0"),
        ("2023-06-01", "USD", "100.0"),
    ]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![
        make_trade("VOO", "2023-01-10", dec!(5), dec!(100.0), Currency::USD),
        make_trade("VOO", "2023-02-10", dec!(5), dec!(110.0), Currency::USD),
        make_trade("VOO", "2023-06-01", dec!(-8), dec!(120.0), Currency::USD),
    ];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].quantity, dec!(5));
    assert_eq!(entries[0].purchase_price, dec!(100.0));
    assert_eq!(
        entries[0].purchase_date,
        NaiveDate::from_ymd_opt(2023, 1, 10).unwrap()
    );

    assert_eq!(entries[1].quantity, dec!(3));
    assert_eq!(entries[1].purchase_price, dec!(110.0));
    assert_eq!(
        entries[1].purchase_date,
        NaiveDate::from_ymd_opt(2023, 2, 10).unwrap()
    );
}

#[test]
fn test_fifo_empty_inventory_zero_cost_basis() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2023-06-01", "USD", "100.0")]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![make_trade(
        "TSLA",
        "2023-06-01",
        dec!(-10),
        dec!(200.0),
        Currency::USD,
    )];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].purchase_price, dec!(0));
    assert_eq!(entries[0].purchase_date, entries[0].sale_date);
}

#[test]
fn test_ten_year_exemption() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2010-03-15", "USD", "80.0"),
        ("2020-04-01", "USD", "120.0"),
    ]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![
        make_trade("IBM", "2010-03-15", dec!(10), dec!(100.0), Currency::USD),
        make_trade("IBM", "2020-04-01", dec!(-10), dec!(200.0), Currency::USD),
    ];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].is_tax_exempt);
}

#[test]
fn test_ten_year_exemption_not_reached() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2015-03-15", "USD", "80.0"),
        ("2024-03-14", "USD", "120.0"),
    ]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![
        make_trade("IBM", "2015-03-15", dec!(10), dec!(100.0), Currency::USD),
        make_trade("IBM", "2024-03-14", dec!(-10), dec!(200.0), Currency::USD),
    ];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].is_tax_exempt);
}

#[test]
fn test_ten_year_leap_day_feb29() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2016-02-29", "USD", "100.0"),
        ("2026-02-28", "USD", "100.0"),
    ]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    // Feb 29, 2016 + 10 years = Feb 28, 2026 (non-leap year)
    let txns = vec![
        make_trade("MSFT", "2016-02-29", dec!(10), dec!(50.0), Currency::USD),
        make_trade("MSFT", "2026-02-28", dec!(-10), dec!(300.0), Currency::USD),
    ];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].is_tax_exempt);
}

#[test]
fn test_force_mode_no_rate_errors_when_cache_empty() {
    let (_tmp, storage, cal) = setup_with_rates(&[]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::with_force(&nbs, true);

    let txns = vec![
        make_trade("AAPL", "2023-01-15", dec!(10), dec!(150.0), Currency::USD),
        make_trade("AAPL", "2023-06-15", dec!(-10), dec!(170.0), Currency::USD),
    ];

    let result = calc.process_trades(&txns);
    assert!(
        result.is_err(),
        "force mode with empty cache should still error"
    );
}

#[test]
fn test_force_mode_uses_nearest_cached_rate() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-01-15", "USD", "117.50"),
        // No rate on 2023-06-15, but there's one from Jan that the force lookback should find
    ]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::with_force(&nbs, true);

    let txns = vec![
        make_trade("AAPL", "2023-01-15", dec!(10), dec!(150.0), Currency::USD),
        make_trade("AAPL", "2023-06-15", dec!(-10), dec!(170.0), Currency::USD),
    ];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 1);
    // Force mode should use cached rate from 2023-01-15 for the sale date
    assert_eq!(entries[0].sale_exchange_rate, dec!(117.5));
}

#[test]
fn test_normal_mode_no_rate_errors() {
    let (_tmp, storage, cal) = setup_with_rates(&[]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![
        make_trade("AAPL", "2023-01-15", dec!(10), dec!(150.0), Currency::USD),
        make_trade("AAPL", "2023-06-15", dec!(-10), dec!(170.0), Currency::USD),
    ];

    let result = calc.process_trades(&txns);
    assert!(result.is_err());
}

#[test]
fn test_non_trade_transactions_ignored() {
    let (_tmp, storage, cal) = setup_with_rates(&[("2023-03-15", "USD", "100.0")]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![Transaction {
        transaction_id: "div-1".into(),
        date: NaiveDate::from_ymd_opt(2023, 3, 15).unwrap(),
        r#type: TransactionType::Dividend,
        symbol: "AAPL".into(),
        description: String::new(),
        quantity: Decimal::ZERO,
        price: Decimal::ZERO,
        amount: dec!(50.0),
        currency: Currency::USD,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
    }];

    let entries = calc.process_trades(&txns).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn test_rsd_amounts_correctly_computed() {
    let (_tmp, storage, cal) = setup_with_rates(&[
        ("2023-01-15", "USD", "117.50"),
        ("2023-06-15", "USD", "108.00"),
    ]);

    let nbs = nbs_offline(&storage, &cal);
    let calc = TaxCalculator::new(&nbs);

    let txns = vec![
        make_trade("A", "2023-01-15", dec!(10), dec!(100.0), Currency::USD),
        make_trade("A", "2023-06-15", dec!(-10), dec!(120.0), Currency::USD),
    ];

    let entries = calc.process_trades(&txns).unwrap();
    assert_eq!(entries.len(), 1);
    let e = &entries[0];

    // sale: 10 * 120 * 108 = 129600
    assert_eq!(e.sale_value_rsd, dec!(129600.00));
    // purchase: 10 * 100 * 117.5 = 117500
    assert_eq!(e.purchase_value_rsd, dec!(117500.00));
    assert_eq!(e.capital_gain_rsd, dec!(12100.00));
}
