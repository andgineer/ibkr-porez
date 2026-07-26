use anyhow::Result;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::io::BufRead;
use std::str::FromStr;
use tracing::debug;

use crate::models::{Currency, Transaction, TransactionType};

/// Parse an IBKR Activity Statement CSV and return extracted transactions.
///
/// The CSV is section-based: rows are grouped under section names like "Trades",
/// "Dividends", "Withholding Tax". Each section has a "Header" row followed by
/// "Data" rows.
pub fn parse_csv_activity<R: BufRead>(reader: R) -> Result<Vec<Transaction>> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(reader);

    let mut transactions = Vec::new();
    let mut trades_header: Option<HashMap<String, usize>> = None;
    let mut divs_header: Option<HashMap<String, usize>> = None;

    for result in csv_reader.records() {
        let Ok(record) = result else { continue };

        if record.is_empty() {
            continue;
        }

        let section = record.get(0).unwrap_or("");
        let discriminator = record.get(1).unwrap_or("");

        if discriminator == "Header" {
            let header_map: HashMap<String, usize> = record
                .iter()
                .enumerate()
                .map(|(i, col)| (col.to_string(), i))
                .collect();

            match section {
                "Trades" => trades_header = Some(header_map),
                "Dividends" | "Withholding Tax" => divs_header = Some(header_map),
                _ => {}
            }
            continue;
        }

        if discriminator == "Data" {
            match section {
                "Trades" => {
                    if let Some(ref header) = trades_header
                        && let Some(t) = parse_trade_row(&record, header)
                    {
                        transactions.push(t);
                    }
                }
                "Dividends" | "Withholding Tax" => {
                    if let Some(ref header) = divs_header
                        && let Some(t) = parse_dividend_row(&record, header, section)
                    {
                        transactions.push(t);
                    }
                }
                _ => {}
            }
        }
    }

    debug!(count = transactions.len(), "parsed CSV transactions");
    Ok(transactions)
}

fn get_field<'a>(
    record: &'a csv::StringRecord,
    header: &HashMap<String, usize>,
    name: &str,
) -> Option<&'a str> {
    let &idx = header.get(name)?;
    let val = record.get(idx)?;
    if val.is_empty() { None } else { Some(val) }
}

fn parse_trade_row(
    record: &csv::StringRecord,
    header: &HashMap<String, usize>,
) -> Option<Transaction> {
    let asset_cat = get_field(record, header, "Asset Category").unwrap_or("");
    if !asset_cat.is_empty() && asset_cat != "Stocks" && asset_cat != "Equity" {
        return None;
    }

    let symbol = get_field(record, header, "Symbol")?;
    let dt_str = get_field(record, header, "Date/Time")?;
    let qty_str = get_field(record, header, "Quantity")?;
    let price_str = get_field(record, header, "T. Price")?;
    let curr_str = get_field(record, header, "Currency")?;
    let proceeds_str = get_field(record, header, "Proceeds").unwrap_or("0");

    let date = parse_csv_date(dt_str)?;
    let currency = Currency::from_code(curr_str)?;
    let quantity = Decimal::from_str(&qty_str.replace(',', "")).ok()?;
    let price = Decimal::from_str(&price_str.replace(',', "")).ok()?;
    let proceeds = Decimal::from_str(&proceeds_str.replace(',', "")).unwrap_or_default();

    let tx_id = get_field(record, header, "Transaction ID").map_or_else(
        || format!("csv-{symbol}-{dt_str}-{qty_str}-{price_str}"),
        String::from,
    );

    Some(Transaction {
        transaction_id: tx_id,
        date,
        r#type: TransactionType::Trade,
        symbol: symbol.to_string(),
        description: format!("Imported Trade {symbol}"),
        quantity,
        price,
        amount: proceeds,
        currency,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
        action_id: None,
    })
}

fn parse_dividend_row(
    record: &csv::StringRecord,
    header: &HashMap<String, usize>,
    section: &str,
) -> Option<Transaction> {
    let dt_str = get_field(record, header, "Date")?;
    let amount_str = get_field(record, header, "Amount")?;
    let curr_str = get_field(record, header, "Currency")?;
    let symbol = get_field(record, header, "Symbol").unwrap_or("UNKNOWN");
    let description = get_field(record, header, "Description").unwrap_or(section);

    let date = NaiveDate::parse_from_str(dt_str, "%Y-%m-%d").ok()?;
    let currency = Currency::from_code(curr_str)?;
    let amount = Decimal::from_str(&amount_str.replace(',', "")).ok()?;

    let tx_type = if section == "Dividends" {
        TransactionType::Dividend
    } else {
        TransactionType::WithholdingTax
    };

    let tx_id = get_field(record, header, "Transaction ID").map_or_else(
        || format!("csv-{section}-{dt_str}-{amount_str}-{curr_str}"),
        String::from,
    );

    Some(Transaction {
        transaction_id: tx_id,
        date,
        r#type: tx_type,
        symbol: symbol.to_string(),
        description: description.to_string(),
        quantity: Decimal::ZERO,
        price: Decimal::ZERO,
        amount,
        currency,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
        action_id: None,
    })
}

fn parse_csv_date(s: &str) -> Option<NaiveDate> {
    let date_part = if let Some((d, _)) = s.split_once(',') {
        d.trim()
    } else if let Some((d, _)) = s.split_once(' ') {
        d
    } else {
        s
    };
    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}
