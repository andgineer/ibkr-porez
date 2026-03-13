use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use rust_decimal_macros::dec;

use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::Currency;
use ibkr_porez::nbs::NBSClient;
use ibkr_porez::storage::Storage;

fn calendar() -> HolidayCalendar {
    HolidayCalendar::load_embedded()
}

// ---------------------------------------------------------------------------
// RSD shortcut (no HTTP needed)
// ---------------------------------------------------------------------------

#[test]
fn test_rsd_returns_one() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let cal = calendar();
    let client = NBSClient::with_base_url(&storage, &cal, "http://127.0.0.1:1");

    let rate = client
        .get_rate(
            NaiveDate::from_ymd_opt(2025, 6, 15).unwrap(),
            &Currency::RSD,
        )
        .unwrap();
    assert_eq!(rate, Some(dec!(1)));
}

// ---------------------------------------------------------------------------
// Fetch and cache via mock HTTP
// ---------------------------------------------------------------------------

#[test]
fn test_fetch_rate_and_cache() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let cal = calendar();

    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/currencies/usd/rates/2025-01-09")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"exchange_middle": 117.25}"#)
        .create();

    let client = NBSClient::with_base_url(&storage, &cal, &server.url());
    let date = NaiveDate::from_ymd_opt(2025, 1, 9).unwrap(); // Thursday, no holiday

    let rate = client.get_rate(date, &Currency::USD).unwrap();
    assert_eq!(rate, Some(dec!(117.25)));
    mock.assert();

    // Verify it was cached
    let cached = storage.get_exchange_rate(date, &Currency::USD);
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().rate, dec!(117.25));
}

// ---------------------------------------------------------------------------
// Weekend fallback
// ---------------------------------------------------------------------------

#[test]
fn test_weekend_fallback() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let cal = calendar();

    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/currencies/eur/rates/2025-01-03")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"exchange_middle": 117.05}"#)
        .create();

    let client = NBSClient::with_base_url(&storage, &cal, &server.url());
    let sunday = NaiveDate::from_ymd_opt(2025, 1, 5).unwrap();

    let rate = client.get_rate(sunday, &Currency::EUR).unwrap();
    assert_eq!(rate, Some(dec!(117.05)));
    mock.assert();

    assert!(storage.get_exchange_rate(sunday, &Currency::EUR).is_some());
    assert!(
        storage
            .get_exchange_rate(NaiveDate::from_ymd_opt(2025, 1, 3).unwrap(), &Currency::EUR)
            .is_some()
    );
}

// ---------------------------------------------------------------------------
// Holiday fallback
// ---------------------------------------------------------------------------

#[test]
fn test_holiday_fallback() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let cal = calendar();

    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/currencies/usd/rates/2025-01-06")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"exchange_middle": 116.50}"#)
        .create();

    let client = NBSClient::with_base_url(&storage, &cal, &server.url());
    let orthodox_christmas = NaiveDate::from_ymd_opt(2025, 1, 7).unwrap();

    let rate = client.get_rate(orthodox_christmas, &Currency::USD).unwrap();
    assert_eq!(rate, Some(dec!(116.50)));
    mock.assert();
}

// ---------------------------------------------------------------------------
// Cache hit (no HTTP call)
// ---------------------------------------------------------------------------

#[test]
fn test_cache_hit_no_http() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let cal = calendar();

    let date = NaiveDate::from_ymd_opt(2025, 3, 10).unwrap();
    storage
        .save_exchange_rate(&ibkr_porez::models::ExchangeRate {
            date,
            currency: Currency::USD,
            rate: dec!(118.0),
        })
        .unwrap();

    let mut server = mockito::Server::new();
    let mock = server.mock("GET", mockito::Matcher::Any).expect(0).create();

    let client = NBSClient::with_base_url(&storage, &cal, &server.url());
    let rate = client.get_rate(date, &Currency::USD).unwrap();
    assert_eq!(rate, Some(dec!(118.0)));
    mock.assert();
}

// ---------------------------------------------------------------------------
// API 404 triggers lookback
// ---------------------------------------------------------------------------

#[test]
fn test_api_error_triggers_lookback() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let cal = calendar();

    let mut server = mockito::Server::new();
    let mock_fail = server
        .mock("GET", "/currencies/usd/rates/2025-01-08")
        .with_status(404)
        .expect(1)
        .create();
    let mock_ok = server
        .mock("GET", "/currencies/usd/rates/2025-01-06")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"exchange_middle": 116.00}"#)
        .create();

    let client = NBSClient::with_base_url(&storage, &cal, &server.url());
    let date = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();

    let rate = client.get_rate(date, &Currency::USD).unwrap();
    assert_eq!(rate, Some(dec!(116.00)));
    mock_fail.assert();
    mock_ok.assert();
}

// ---------------------------------------------------------------------------
// Missing holiday year returns error
// ---------------------------------------------------------------------------

#[test]
fn test_missing_holiday_year_errors() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let cal = HolidayCalendar::empty(); // no holiday data at all

    let mut server = mockito::Server::new();
    let _mock = server.mock("GET", mockito::Matcher::Any).expect(0).create();

    let client = NBSClient::with_base_url(&storage, &cal, &server.url());
    let date = NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(); // holiday, but no data

    let result = client.get_rate(date, &Currency::USD);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("no holiday data for year 2025"),
        "unexpected error: {err_msg}"
    );
}

// ---------------------------------------------------------------------------
// Fallback mode allows missing year
// ---------------------------------------------------------------------------

#[test]
fn test_fallback_mode_allows_missing_year() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let mut cal = HolidayCalendar::empty();
    cal.set_fallback(true);

    let mut server = mockito::Server::new();
    // Jan 7 is Orthodox Christmas (fallback knows this), should skip to Jan 6
    let mock = server
        .mock("GET", "/currencies/usd/rates/2025-01-06")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"exchange_middle": 116.50}"#)
        .create();

    let client = NBSClient::with_base_url(&storage, &cal, &server.url());
    let date = NaiveDate::from_ymd_opt(2025, 1, 7).unwrap();

    let rate = client.get_rate(date, &Currency::USD).unwrap();
    assert_eq!(rate, Some(dec!(116.50)));
    mock.assert();
}
