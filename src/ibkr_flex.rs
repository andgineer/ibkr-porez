use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use tracing::debug;

use crate::models::{Currency, Transaction, TransactionType};

const FLEX_URL_REQUEST: &str =
    "https://ndcdyn.interactivebrokers.com/Universal/servlet/FlexStatementService.SendRequest";
const FLEX_URL_GET: &str =
    "https://ndcdyn.interactivebrokers.com/Universal/servlet/FlexStatementService.GetStatement";
const VERSION: &str = "3";

const POLL_DELAY_BUSY: std::time::Duration = std::time::Duration::from_secs(5);
const POLL_DELAY_THROTTLED: std::time::Duration = std::time::Duration::from_secs(10);
const MAX_POLL_ATTEMPTS: u32 = 5;
const FATAL_ERROR_CODES: &[&str] = &[
    "1010", // Legacy Flex Queries no longer supported
    "1011", // Service account is inactive
    "1012", // Token has expired
    "1013", // IP restriction
    "1014", // Query is invalid
    "1015", // Token is invalid
    "1016", // Account is invalid
    "1017", // Reference code is invalid
    "1020", // Invalid request or unable to validate request
];

pub struct IBKRClient {
    token: String,
    query_id: String,
    request_url: String,
    get_url: String,
    http: reqwest::blocking::Client,
    /// `None` in production — delay per error code. Set in tests to avoid real sleeps.
    poll_delay_override: Option<std::time::Duration>,
    max_poll_attempts: u32,
}

impl IBKRClient {
    #[must_use]
    pub fn new(token: &str, query_id: &str) -> Self {
        Self {
            token: token.to_string(),
            query_id: query_id.to_string(),
            request_url: FLEX_URL_REQUEST.to_string(),
            get_url: FLEX_URL_GET.to_string(),
            http: build_http_client(std::time::Duration::from_secs(30)),
            poll_delay_override: None,
            max_poll_attempts: MAX_POLL_ATTEMPTS,
        }
    }

    #[must_use]
    pub fn with_base_url(token: &str, query_id: &str, base_url: &str) -> Self {
        Self {
            token: token.to_string(),
            query_id: query_id.to_string(),
            request_url: format!("{base_url}/SendRequest"),
            get_url: format!("{base_url}/GetStatement"),
            http: build_http_client(std::time::Duration::from_secs(30)),
            poll_delay_override: None,
            max_poll_attempts: MAX_POLL_ATTEMPTS,
        }
    }

    pub fn fetch_latest_report(&self) -> Result<String> {
        let (reference_code, base_url) = self.send_statement_request()?;
        self.poll_for_statement(&reference_code, &base_url)
    }

    /// `SendRequest` once — no retries. If it fails for any reason the caller
    /// (GUI hourly retry or CLI invocation) decides whether to try again.
    fn send_statement_request(&self) -> Result<(String, String)> {
        let resp = self
            .http
            .get(&self.request_url)
            .query(&[
                ("t", self.token.as_str()),
                ("q", self.query_id.as_str()),
                ("v", VERSION),
            ])
            .send()
            .context("IBKR SendRequest failed")?;
        resp.error_for_status_ref()
            .context("IBKR SendRequest HTTP error")?;
        let body = resp.text()?;

        let req_resp: XmlRequestResponse =
            quick_xml::de::from_str(&body).context("Failed to parse IBKR SendRequest response")?;

        if let Some(code) = &req_resp.error_code {
            let msg = req_resp.error_message.as_deref().unwrap_or("Unknown");
            bail!("IBKR API Error {code}: {msg}");
        }

        let reference_code = req_resp
            .reference_code
            .context("No ReferenceCode in IBKR response")?;
        let base_url = req_resp
            .url
            .filter(|u| !u.is_empty())
            .unwrap_or_else(|| self.get_url.clone());

        Ok((reference_code, base_url))
    }

    /// Poll the same `ReferenceCode` up to `max_poll_attempts` times.
    /// Retries on transient IBKR error codes and network errors; aborts on fatal codes.
    fn poll_for_statement(&self, reference_code: &str, base_url: &str) -> Result<String> {
        let mut last_err: Option<anyhow::Error> = None;
        for attempt in 0..self.max_poll_attempts {
            let body = self
                .http
                .get(base_url)
                .query(&[
                    ("q", reference_code),
                    ("t", self.token.as_str()),
                    ("v", VERSION),
                ])
                .send()
                .context("IBKR GetStatement request failed")
                .and_then(|r| {
                    r.error_for_status_ref()
                        .context("IBKR GetStatement HTTP error")?;
                    Ok(r.text()?)
                });

            match body {
                Err(e) => {
                    debug!(attempt, error = %e, "GetStatement failed, retrying");
                    if attempt + 1 < self.max_poll_attempts {
                        std::thread::sleep(self.poll_delay_override.unwrap_or(POLL_DELAY_BUSY));
                    }
                    last_err = Some(e);
                }
                Ok(body) => {
                    if body.contains("<ErrorCode>")
                        && let Ok(err_resp) = quick_xml::de::from_str::<XmlErrorResponse>(&body)
                        && let Some(code) = &err_resp.error_code
                    {
                        let msg = err_resp.error_message.as_deref().unwrap_or("Unknown");
                        let e = anyhow::anyhow!("IBKR API Error {code}: {msg}");
                        if FATAL_ERROR_CODES.contains(&code.as_str()) {
                            return Err(e);
                        }
                        let delay = if code == "1018" {
                            POLL_DELAY_THROTTLED
                        } else {
                            POLL_DELAY_BUSY
                        };
                        debug!(attempt, error = %e, "GetStatement not ready, retrying");
                        if attempt + 1 < self.max_poll_attempts {
                            std::thread::sleep(self.poll_delay_override.unwrap_or(delay));
                        }
                        last_err = Some(e);
                        continue;
                    }
                    return Ok(body);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| {
            anyhow::anyhow!(
                "GetStatement: not ready after {} attempts",
                self.max_poll_attempts
            )
        }))
    }

    #[cfg(test)]
    fn with_poll_params(mut self, delay_override: std::time::Duration, max_attempts: u32) -> Self {
        self.poll_delay_override = Some(delay_override);
        self.max_poll_attempts = max_attempts;
        self
    }
}

fn build_http_client(timeout: std::time::Duration) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .expect("TLS backend unavailable")
}

// ---------------------------------------------------------------------------
// XML parsing (standalone — no HTTP needed)
// ---------------------------------------------------------------------------

/// Parse an IBKR Flex Query XML report into a list of transactions.
pub fn parse_flex_report(xml: &str) -> Result<Vec<Transaction>> {
    if xml.contains("<ErrorCode>")
        && let Ok(err) = quick_xml::de::from_str::<XmlErrorResponse>(xml)
        && let Some(code) = &err.error_code
    {
        let msg = err.error_message.as_deref().unwrap_or("Unknown error");
        bail!("Flex Query Failed: {code} - {msg}");
    }

    let response: XmlFlexQueryResponse =
        quick_xml::de::from_str(xml).context("Failed to parse Flex Query XML")?;

    let mut transactions = Vec::new();
    for stmt in &response.flex_statements.statements {
        if let Some(trades) = &stmt.trades {
            for trade in &trades.items {
                if let Some(t) = convert_trade(trade) {
                    transactions.push(t);
                }
            }
        }
        if let Some(cash) = &stmt.cash_transactions {
            for ct in &cash.items {
                if let Some(t) = convert_cash_transaction(ct) {
                    transactions.push(t);
                }
            }
        }
    }

    debug!(
        count = transactions.len(),
        "parsed flex report transactions"
    );
    Ok(transactions)
}

fn parse_ibkr_date(s: &str) -> Option<NaiveDate> {
    let clean = s.split(';').next().unwrap_or(s);
    if clean.contains('-') {
        NaiveDate::parse_from_str(clean, "%Y-%m-%d").ok()
    } else {
        NaiveDate::parse_from_str(clean, "%Y%m%d").ok()
    }
}

fn convert_trade(el: &XmlTrade) -> Option<Transaction> {
    let symbol = non_empty(el.symbol.as_ref())?;
    let currency_str = non_empty(el.currency.as_ref())?;
    let quantity_str = non_empty(el.quantity.as_ref())?;
    let price_str = non_empty(el.trade_price.as_ref())?;
    let date_str = non_empty(el.trade_date.as_ref())?;
    let trade_id = non_empty(el.trade_id.as_ref())?;

    let date = parse_ibkr_date(date_str)?;
    let currency = Currency::from_code(currency_str)?;
    let quantity = Decimal::from_str(quantity_str).ok()?;
    let price = Decimal::from_str(price_str).ok()?;

    let amount_str = el
        .fifo_pnl_realized
        .as_deref()
        .or(el.proceeds.as_deref())
        .unwrap_or("0");
    let amount = Decimal::from_str(amount_str).unwrap_or_default();

    let open_date = el.orig_trade_date.as_deref().and_then(parse_ibkr_date);
    let open_price = el
        .orig_trade_price
        .as_deref()
        .and_then(|s| Decimal::from_str(s).ok());

    Some(Transaction {
        transaction_id: trade_id.to_string(),
        date,
        r#type: TransactionType::Trade,
        symbol: symbol.to_string(),
        description: el.description.clone().unwrap_or_default(),
        quantity,
        price,
        amount,
        currency,
        open_date,
        open_price,
        exchange_rate: None,
        amount_rsd: None,
    })
}

fn convert_cash_transaction(el: &XmlCashTransaction) -> Option<Transaction> {
    let type_str = el.r#type.as_deref().unwrap_or("");
    let tx_type = match type_str {
        "Dividends" | "Payment In Lieu Of Dividends" => TransactionType::Dividend,
        "Withholding Tax" => TransactionType::WithholdingTax,
        "Broker Interest Paid" | "Broker Interest Received" => TransactionType::Interest,
        _ => return None,
    };

    let currency_str = non_empty(el.currency.as_ref())?;
    let amount_str = non_empty(el.amount.as_ref())?;
    let date_str = non_empty(el.date_time.as_ref())?;
    let tx_id = non_empty(el.transaction_id.as_ref())?;

    let date = parse_ibkr_date(date_str)?;
    let currency = Currency::from_code(currency_str)?;
    let amount = Decimal::from_str(amount_str).ok()?;

    Some(Transaction {
        transaction_id: tx_id.to_string(),
        date,
        r#type: tx_type,
        symbol: el.symbol.clone().unwrap_or_default(),
        description: el.description.clone().unwrap_or_default(),
        quantity: Decimal::ZERO,
        price: Decimal::ZERO,
        amount,
        currency,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
    })
}

fn non_empty(opt: Option<&String>) -> Option<&str> {
    opt.map(String::as_str).filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// XML deserialization structs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct XmlRequestResponse {
    #[serde(rename = "ReferenceCode")]
    reference_code: Option<String>,
    #[serde(rename = "Url")]
    url: Option<String>,
    #[serde(rename = "ErrorCode")]
    error_code: Option<String>,
    #[serde(rename = "ErrorMessage")]
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XmlErrorResponse {
    #[serde(rename = "ErrorCode")]
    error_code: Option<String>,
    #[serde(rename = "ErrorMessage")]
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XmlFlexQueryResponse {
    #[serde(rename = "FlexStatements")]
    flex_statements: XmlFlexStatements,
}

#[derive(Debug, Deserialize)]
struct XmlFlexStatements {
    #[serde(rename = "FlexStatement", default)]
    statements: Vec<XmlFlexStatement>,
}

#[derive(Debug, Deserialize)]
struct XmlFlexStatement {
    #[serde(rename = "Trades")]
    trades: Option<XmlTrades>,
    #[serde(rename = "CashTransactions")]
    cash_transactions: Option<XmlCashTransactions>,
}

#[derive(Debug, Deserialize)]
struct XmlTrades {
    #[serde(rename = "Trade", default)]
    items: Vec<XmlTrade>,
}

#[derive(Debug, Deserialize)]
struct XmlCashTransactions {
    #[serde(rename = "CashTransaction", default)]
    items: Vec<XmlCashTransaction>,
}

#[derive(Debug, Deserialize)]
struct XmlTrade {
    #[serde(rename = "@symbol")]
    symbol: Option<String>,
    #[serde(rename = "@currency")]
    currency: Option<String>,
    #[serde(rename = "@quantity")]
    quantity: Option<String>,
    #[serde(rename = "@tradePrice")]
    trade_price: Option<String>,
    #[serde(rename = "@tradeDate")]
    trade_date: Option<String>,
    #[serde(rename = "@tradeID")]
    trade_id: Option<String>,
    #[serde(rename = "@fifoPnlRealized")]
    fifo_pnl_realized: Option<String>,
    #[serde(rename = "@proceeds")]
    proceeds: Option<String>,
    #[serde(rename = "@origTradeDate")]
    orig_trade_date: Option<String>,
    #[serde(rename = "@origTradePrice")]
    orig_trade_price: Option<String>,
    #[serde(rename = "@description")]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XmlCashTransaction {
    #[serde(rename = "@type")]
    r#type: Option<String>,
    #[serde(rename = "@symbol")]
    symbol: Option<String>,
    #[serde(rename = "@currency")]
    currency: Option<String>,
    #[serde(rename = "@amount")]
    amount: Option<String>,
    #[serde(rename = "@dateTime")]
    date_time: Option<String>,
    #[serde(rename = "@transactionID")]
    transaction_id: Option<String>,
    #[serde(rename = "@description")]
    description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ibkr_date_yyyymmdd() {
        assert_eq!(
            parse_ibkr_date("20230115"),
            Some(NaiveDate::from_ymd_opt(2023, 1, 15).unwrap())
        );
    }

    #[test]
    fn parse_ibkr_date_hyphenated() {
        assert_eq!(
            parse_ibkr_date("2023-01-15"),
            Some(NaiveDate::from_ymd_opt(2023, 1, 15).unwrap())
        );
    }

    #[test]
    fn parse_ibkr_date_with_semicolon() {
        assert_eq!(
            parse_ibkr_date("20230601;120000"),
            Some(NaiveDate::from_ymd_opt(2023, 6, 1).unwrap())
        );
    }

    #[test]
    fn parse_ibkr_date_invalid() {
        assert!(parse_ibkr_date("not-a-date").is_none());
        assert!(parse_ibkr_date("").is_none());
    }

    #[test]
    fn non_empty_filters() {
        assert_eq!(non_empty(Some(&String::new())), None);
        assert_eq!(non_empty(None), None);
        assert_eq!(non_empty(Some(&"hello".to_string())), Some("hello"));
    }

    #[test]
    fn convert_trade_minimal() {
        let trade = XmlTrade {
            symbol: Some("AAPL".into()),
            currency: Some("USD".into()),
            quantity: Some("10".into()),
            trade_price: Some("150.00".into()),
            trade_date: Some("20230101".into()),
            trade_id: Some("T1".into()),
            fifo_pnl_realized: None,
            proceeds: None,
            orig_trade_date: None,
            orig_trade_price: None,
            description: Some("Apple".into()),
        };
        let tx = convert_trade(&trade).unwrap();
        assert_eq!(tx.symbol, "AAPL");
        assert_eq!(tx.amount, Decimal::ZERO);
    }

    #[test]
    fn convert_trade_missing_symbol() {
        let trade = XmlTrade {
            symbol: None,
            currency: Some("USD".into()),
            quantity: Some("10".into()),
            trade_price: Some("150.00".into()),
            trade_date: Some("20230101".into()),
            trade_id: Some("T1".into()),
            fifo_pnl_realized: None,
            proceeds: None,
            orig_trade_date: None,
            orig_trade_price: None,
            description: None,
        };
        assert!(convert_trade(&trade).is_none());
    }

    #[test]
    fn convert_cash_unknown_type() {
        let ct = XmlCashTransaction {
            r#type: Some("Fee".into()),
            symbol: Some("AAPL".into()),
            currency: Some("USD".into()),
            amount: Some("10".into()),
            date_time: Some("20230101".into()),
            transaction_id: Some("C1".into()),
            description: None,
        };
        assert!(convert_cash_transaction(&ct).is_none());
    }

    #[test]
    fn convert_cash_dividend() {
        let ct = XmlCashTransaction {
            r#type: Some("Dividends".into()),
            symbol: Some("AAPL".into()),
            currency: Some("USD".into()),
            amount: Some("50.00".into()),
            date_time: Some("20230315".into()),
            transaction_id: Some("D1".into()),
            description: Some("Apple div".into()),
        };
        let tx = convert_cash_transaction(&ct).unwrap();
        assert_eq!(tx.r#type, TransactionType::Dividend);
        assert_eq!(tx.amount, Decimal::from_str("50.00").unwrap());
    }

    #[test]
    fn convert_cash_withholding_tax() {
        let ct = XmlCashTransaction {
            r#type: Some("Withholding Tax".into()),
            symbol: Some("AAPL".into()),
            currency: Some("USD".into()),
            amount: Some("-7.50".into()),
            date_time: Some("20230315".into()),
            transaction_id: Some("W1".into()),
            description: None,
        };
        let tx = convert_cash_transaction(&ct).unwrap();
        assert_eq!(tx.r#type, TransactionType::WithholdingTax);
    }

    fn flex_xml_report() -> &'static str {
        r#"<FlexQueryResponse>
          <FlexStatements>
            <FlexStatement>
              <Trades>
                <Trade symbol="AAPL" currency="USD" quantity="10" tradePrice="150.00"
                       tradeDate="20250109" tradeID="T1" fifoPnlRealized="100.00"
                       description="Apple Inc" />
              </Trades>
              <CashTransactions>
                <CashTransaction type="Dividends" symbol="AAPL" currency="USD"
                                 amount="25.00" dateTime="20250109" transactionID="D1"
                                 description="AAPL dividend" />
              </CashTransactions>
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

    fn mock_send_request_success(server: &mut mockito::Server) -> mockito::Mock {
        server
            .mock("GET", send_request_matcher())
            .with_status(200)
            .with_body(format!(
                "<FlexStatementResponse>\
                   <Status>Success</Status>\
                   <ReferenceCode>REF123</ReferenceCode>\
                   <Url>{}/GetStatement</Url>\
                 </FlexStatementResponse>",
                server.url()
            ))
            .expect(1)
            .create()
    }

    #[test]
    fn ibkr_client_fetch_success() {
        let xml = flex_xml_report();
        let mut server = mockito::Server::new();
        let request_mock = mock_send_request_success(&mut server);
        let get_mock = server
            .mock("GET", get_statement_matcher())
            .with_status(200)
            .with_body(xml)
            .create();

        let client = IBKRClient::with_base_url("tok", "qid", &server.url());
        let result = client.fetch_latest_report().unwrap();
        assert!(result.contains("AAPL"));
        request_mock.assert();
        get_mock.assert();
    }

    #[test]
    fn ibkr_client_send_request_error_fails_immediately() {
        // SendRequest IBKR error → fails immediately, no retry
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", send_request_matcher())
            .with_status(200)
            .with_body(
                "<FlexStatementResponse>\
                   <ErrorCode>1019</ErrorCode>\
                   <ErrorMessage>Token expired</ErrorMessage>\
                 </FlexStatementResponse>",
            )
            .expect(1)
            .create();

        let client = IBKRClient::with_base_url("tok", "qid", &server.url());
        let err = client.fetch_latest_report().unwrap_err();
        assert!(err.to_string().contains("1019"));
        assert!(err.to_string().contains("Token expired"));
        mock.assert();
    }

    #[test]
    fn ibkr_client_get_statement_fatal_error_aborts_immediately() {
        // Fatal IBKR error on GetStatement → no retry
        let mut server = mockito::Server::new();
        let request_mock = mock_send_request_success(&mut server);
        let get_mock = server
            .mock("GET", get_statement_matcher())
            .with_status(200)
            .with_body(
                "<FlexStatementResponse>\
                   <ErrorCode>1012</ErrorCode>\
                   <ErrorMessage>Token has expired</ErrorMessage>\
                 </FlexStatementResponse>",
            )
            .expect(1)
            .create();

        let client = IBKRClient::with_base_url("tok", "qid", &server.url())
            .with_poll_params(std::time::Duration::ZERO, 5);
        let err = client.fetch_latest_report().unwrap_err();
        assert!(err.to_string().contains("1012"));
        request_mock.assert();
        get_mock.assert();
    }

    #[test]
    fn ibkr_client_get_statement_exhausts_retries() {
        // Transient error on every GetStatement → exhausts budget, returns error
        let mut server = mockito::Server::new();
        let request_mock = mock_send_request_success(&mut server);
        let get_mock = server
            .mock("GET", get_statement_matcher())
            .with_status(200)
            .with_body(
                "<FlexStatementResponse>\
                   <ErrorCode>1019</ErrorCode>\
                   <ErrorMessage>Statement generation in progress</ErrorMessage>\
                 </FlexStatementResponse>",
            )
            .expect(3)
            .create();

        let client = IBKRClient::with_base_url("tok", "qid", &server.url())
            .with_poll_params(std::time::Duration::ZERO, 3);
        let err = client.fetch_latest_report().unwrap_err();
        assert!(err.to_string().contains("1019"));
        request_mock.assert();
        get_mock.assert();
    }

    #[test]
    fn ibkr_client_polls_same_reference_code() {
        // All GetStatement calls must carry the ReferenceCode from SendRequest
        let xml = flex_xml_report();
        let mut server = mockito::Server::new();
        let request_mock = mock_send_request_success(&mut server);
        // Verify the same ReferenceCode (REF123) is used on every GetStatement call
        let get_mock = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"/GetStatement\?.*q=REF123".into()),
            )
            .with_status(200)
            .with_body(xml)
            .expect(1)
            .create();

        let client = IBKRClient::with_base_url("tok", "qid", &server.url())
            .with_poll_params(std::time::Duration::ZERO, 5);
        let result = client.fetch_latest_report().unwrap();
        assert!(result.contains("AAPL"));
        request_mock.assert();
        get_mock.assert();
    }

    #[test]
    fn ibkr_client_http_error_retries() {
        // HTTP 500 on GetStatement is retried; SendRequest is called only once
        let mut server = mockito::Server::new();
        let request_mock = mock_send_request_success(&mut server);
        let get_mock = server
            .mock("GET", get_statement_matcher())
            .with_status(500)
            .expect(3)
            .create();

        let client = IBKRClient::with_base_url("tok", "qid", &server.url())
            .with_poll_params(std::time::Duration::ZERO, 3);
        let result = client.fetch_latest_report();
        assert!(result.is_err());
        request_mock.assert();
        get_mock.assert();
    }

    #[test]
    fn ibkr_client_uses_default_get_url_when_response_url_empty() {
        let xml = flex_xml_report();
        let mut server = mockito::Server::new();
        let request_mock = server
            .mock("GET", send_request_matcher())
            .with_status(200)
            .with_body(
                "<FlexStatementResponse>\
                   <Status>Success</Status>\
                   <ReferenceCode>REF123</ReferenceCode>\
                   <Url></Url>\
                 </FlexStatementResponse>",
            )
            .create();
        let get_mock = server
            .mock("GET", get_statement_matcher())
            .with_status(200)
            .with_body(xml)
            .create();

        let client = IBKRClient::with_base_url("tok", "qid", &server.url());
        let result = client.fetch_latest_report().unwrap();
        assert!(result.contains("AAPL"));
        request_mock.assert();
        get_mock.assert();
    }
}
