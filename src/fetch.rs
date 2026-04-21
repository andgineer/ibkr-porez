use anyhow::{Context, Result};
use chrono::Local;
use tracing::info;

use crate::ibkr_flex::{IBKRClient, parse_flex_report};
use crate::models::{Transaction, UserConfig};
use crate::nbs::NBSClient;
use crate::storage::Storage;

pub struct FetchResult {
    pub transactions: Vec<Transaction>,
    pub inserted: usize,
    pub updated: usize,
}

/// Download the latest IBKR Flex report, parse it, save transactions, and
/// pre-fetch NBS exchange rates. This is the data-retrieval subset of `run_sync`.
pub fn fetch_and_import(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    ibkr: &IBKRClient,
) -> Result<FetchResult> {
    validate_ibkr_config(config)?;

    info!("fetching IBKR data…");
    let xml = ibkr
        .fetch_latest_report()
        .context("failed to fetch IBKR Flex Query report")?;

    let report_date = Local::now().date_naive();
    storage
        .save_raw_report(&xml, report_date)
        .with_context(|| storage.io_error_hint())?;

    let transactions = parse_flex_report(&xml)?;
    let (inserted, updated) = storage
        .save_transactions(&transactions)
        .with_context(|| storage.io_error_hint())?;
    info!(inserted, updated, "saved transactions");

    prefetch_rates(storage, nbs, &transactions);

    Ok(FetchResult {
        transactions,
        inserted,
        updated,
    })
}

pub fn validate_ibkr_config(config: &UserConfig) -> Result<()> {
    if config.ibkr_token.is_empty() || config.ibkr_query_id.is_empty() {
        anyhow::bail!(
            "Missing IBKR configuration. Run `ibkr-porez config` first \
             to set your IBKR token and query ID."
        );
    }
    Ok(())
}

fn prefetch_rates(storage: &Storage, nbs: &NBSClient, transactions: &[Transaction]) {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    for txn in transactions {
        let key = (txn.date, txn.currency.clone());
        if seen.insert(key)
            && let Err(e) = nbs.get_rate(txn.date, &txn.currency)
        {
            tracing::debug!(date = %txn.date, error = %e, "rate prefetch failed (non-fatal)");
        }
    }

    let rates = storage.load_rates();
    info!(cached_rates = rates.len(), "rate prefetch complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    use crate::holidays::HolidayCalendar;
    use crate::models::Currency;

    fn make_config(token: &str, query_id: &str) -> UserConfig {
        UserConfig {
            ibkr_token: token.to_string(),
            ibkr_query_id: query_id.to_string(),
            ..UserConfig::default()
        }
    }

    #[test]
    fn validate_ibkr_config_empty_token() {
        let cfg = make_config("", "q1");
        assert!(validate_ibkr_config(&cfg).is_err());
    }

    #[test]
    fn validate_ibkr_config_empty_query_id() {
        let cfg = make_config("t1", "");
        assert!(validate_ibkr_config(&cfg).is_err());
    }

    #[test]
    fn validate_ibkr_config_both_empty() {
        let cfg = make_config("", "");
        assert!(validate_ibkr_config(&cfg).is_err());
    }

    #[test]
    fn validate_ibkr_config_both_present() {
        let cfg = make_config("token", "query");
        assert!(validate_ibkr_config(&cfg).is_ok());
    }

    fn flex_xml_for_mock() -> &'static str {
        r#"<FlexQueryResponse>
          <FlexStatements>
            <FlexStatement>
              <Trades>
                <Trade symbol="MSFT" currency="USD" quantity="5" tradePrice="400.00"
                       tradeDate="20250109" tradeID="T100" fifoPnlRealized="50.00"
                       description="Microsoft" />
              </Trades>
              <CashTransactions />
            </FlexStatement>
          </FlexStatements>
        </FlexQueryResponse>"#
    }

    fn send_request_matcher() -> mockito::Matcher {
        mockito::Matcher::Regex(r"^/SendRequest\?".into())
    }

    fn get_statement_matcher() -> mockito::Matcher {
        mockito::Matcher::Regex(r"^/GetStatement\?".into())
    }

    #[test]
    fn fetch_and_import_with_mock() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let cal = HolidayCalendar::load_embedded();

        let mut ibkr_server = mockito::Server::new();
        let request_mock = ibkr_server
            .mock("GET", send_request_matcher())
            .with_status(200)
            .with_body(format!(
                "<FlexStatementResponse>\
                   <Status>Success</Status>\
                   <ReferenceCode>REF1</ReferenceCode>\
                   <Url>{}/GetStatement</Url>\
                 </FlexStatementResponse>",
                ibkr_server.url()
            ))
            .create();
        let get_mock = ibkr_server
            .mock("GET", get_statement_matcher())
            .with_status(200)
            .with_body(flex_xml_for_mock())
            .create();

        let mut nbs_server = mockito::Server::new();
        let nbs_mock = nbs_server
            .mock("GET", mockito::Matcher::Regex(r"^/currencies/".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"exchange_middle": 117.25}"#)
            .create();

        let nbs = NBSClient::with_base_url(&storage, &cal, &nbs_server.url());
        let ibkr = IBKRClient::with_base_url("tok", "qid", &ibkr_server.url());
        let cfg = make_config("tok", "qid");

        let result = fetch_and_import(&storage, &nbs, &cfg, &ibkr).unwrap();
        assert_eq!(result.transactions.len(), 1);
        assert_eq!(result.inserted, 1);
        assert_eq!(result.transactions[0].symbol, "MSFT");

        request_mock.assert();
        get_mock.assert();
        nbs_mock.assert();
    }

    #[test]
    fn fetch_and_import_rejects_empty_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let cal = HolidayCalendar::load_embedded();
        let nbs = NBSClient::with_base_url(&storage, &cal, "http://127.0.0.1:1");
        let ibkr = IBKRClient::with_base_url("tok", "qid", "http://127.0.0.1:1");
        let cfg = make_config("", "");

        let result = fetch_and_import(&storage, &nbs, &cfg, &ibkr);
        assert!(result.is_err());
    }

    #[test]
    fn prefetch_rates_caches_results() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let cal = HolidayCalendar::load_embedded();

        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/currencies/usd/rates/2025-01-09")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"exchange_middle": 117.25}"#)
            .create();

        let nbs = NBSClient::with_base_url(&storage, &cal, &server.url());
        let date = NaiveDate::from_ymd_opt(2025, 1, 9).unwrap();
        let txns = vec![Transaction {
            transaction_id: "T1".into(),
            date,
            r#type: crate::models::TransactionType::Trade,
            symbol: "AAPL".into(),
            description: String::new(),
            quantity: Decimal::from_str("10").unwrap(),
            price: Decimal::from_str("150").unwrap(),
            amount: Decimal::from_str("100").unwrap(),
            currency: Currency::USD,
            open_date: None,
            open_price: None,
            exchange_rate: None,
            amount_rsd: None,
        }];

        prefetch_rates(&storage, &nbs, &txns);
        mock.assert();

        let cached = storage.get_exchange_rate(date, &Currency::USD);
        assert!(cached.is_some());
    }

    #[test]
    fn prefetch_rates_deduplicates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let cal = HolidayCalendar::load_embedded();

        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/currencies/usd/rates/2025-01-09")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"exchange_middle": 117.0}"#)
            .expect(1)
            .create();

        let nbs = NBSClient::with_base_url(&storage, &cal, &server.url());
        let date = NaiveDate::from_ymd_opt(2025, 1, 9).unwrap();
        let make_txn = |id: &str| Transaction {
            transaction_id: id.into(),
            date,
            r#type: crate::models::TransactionType::Dividend,
            symbol: "AAPL".into(),
            description: String::new(),
            quantity: Decimal::ZERO,
            price: Decimal::ZERO,
            amount: Decimal::from_str("10").unwrap(),
            currency: Currency::USD,
            open_date: None,
            open_price: None,
            exchange_rate: None,
            amount_rsd: None,
        };
        let txns = vec![make_txn("D1"), make_txn("D2")];

        prefetch_rates(&storage, &nbs, &txns);
        mock.assert();
    }
}
