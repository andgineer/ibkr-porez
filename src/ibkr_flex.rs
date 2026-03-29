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
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

pub struct IBKRClient {
    token: String,
    query_id: String,
    http: reqwest::blocking::Client,
}

impl IBKRClient {
    #[must_use]
    pub fn new(token: &str, query_id: &str) -> Self {
        Self {
            token: token.to_string(),
            query_id: query_id.to_string(),
            http: build_http_client(std::time::Duration::from_secs(30)),
        }
    }

    /// Fetch the latest Flex Query report XML from IBKR.
    /// Retries up to 3 times with 2-second delays.
    pub fn fetch_latest_report(&self) -> Result<String> {
        let mut last_err = None;
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                debug!(attempt, "retrying IBKR fetch");
                std::thread::sleep(RETRY_DELAY);
            }
            match self.try_fetch_report() {
                Ok(xml) => return Ok(xml),
                Err(e) => {
                    debug!(attempt, error = %e, "IBKR fetch attempt failed");
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch failed after {MAX_RETRIES} retries")))
    }

    fn try_fetch_report(&self) -> Result<String> {
        let resp = self
            .http
            .get(FLEX_URL_REQUEST)
            .query(&[
                ("t", self.token.as_str()),
                ("q", self.query_id.as_str()),
                ("v", VERSION),
            ])
            .send()
            .context("Failed to send IBKR request")?;
        resp.error_for_status_ref()
            .context("IBKR request endpoint returned error")?;
        let body = resp.text()?;

        let req_resp: XmlRequestResponse =
            quick_xml::de::from_str(&body).context("Failed to parse IBKR request response")?;

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
            .unwrap_or_else(|| FLEX_URL_GET.to_string());

        let resp = self
            .http
            .get(&base_url)
            .query(&[
                ("q", reference_code.as_str()),
                ("t", self.token.as_str()),
                ("v", VERSION),
            ])
            .send()
            .context("Failed to fetch IBKR report")?;
        resp.error_for_status_ref()
            .context("IBKR report endpoint returned error")?;
        Ok(resp.text()?)
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
}
