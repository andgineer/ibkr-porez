use std::collections::BTreeMap;

use anyhow::{Result, bail};
use chrono::Datelike;
use rust_decimal::Decimal;

use crate::models::{Currency, TaxReportEntry, Transaction, TransactionType};
use crate::nbs::NBSClient;
use crate::storage::Storage;
use crate::tax::TaxCalculator;

/// Aggregated row for the monthly breakdown table.
pub struct AggregatedRow {
    pub month: String,
    pub ticker: String,
    pub dividends_rsd: Decimal,
    pub sales_count: usize,
    pub realized_pnl_rsd: Decimal,
}

/// Detailed sale row for the per-ticker breakdown.
pub struct DetailedRow {
    pub sale_date: String,
    pub quantity: Decimal,
    pub sale_price: Decimal,
    pub sale_rate: Decimal,
    pub sale_value_rsd: Decimal,
    pub buy_date: String,
    pub buy_price: Decimal,
    pub buy_rate: Decimal,
    pub buy_value_rsd: Decimal,
    pub gain_rsd: Decimal,
}

pub struct StatResult {
    pub mode: StatMode,
}

pub enum StatMode {
    Aggregated(Vec<AggregatedRow>),
    Detailed {
        rows: Vec<DetailedRow>,
        total_pnl: Decimal,
        title: String,
    },
    Empty(String),
}

pub struct ShowStatistics<'a> {
    storage: &'a Storage,
    nbs: &'a NBSClient<'a>,
}

impl<'a> ShowStatistics<'a> {
    #[must_use]
    pub fn new(storage: &'a Storage, nbs: &'a NBSClient<'a>) -> Self {
        Self { storage, nbs }
    }

    pub fn generate(
        &self,
        year: Option<i32>,
        ticker: Option<&str>,
        month: Option<&str>,
    ) -> Result<StatResult> {
        let transactions = self.storage.load_transactions();
        if transactions.is_empty() {
            return Ok(StatResult {
                mode: StatMode::Empty(
                    "No transactions found. Run `ibkr-porez fetch` or `ibkr-porez sync`.".into(),
                ),
            });
        }

        let tax_calc = TaxCalculator::new(self.nbs);
        let sales_entries = tax_calc.process_trades(&transactions)?;

        let (parsed_year, parsed_month) = parse_month(month)?;
        let mut target_year = year.or(parsed_year);
        let target_month = parsed_month;

        if target_year.is_none()
            && let Some(m) = target_month
        {
            target_year = find_latest_year_for_month(&sales_entries, &transactions, m);
        }

        let show_detailed = ticker.is_some();

        if show_detailed {
            let filtered = filter_entries(&sales_entries, ticker, target_year, target_month);

            if filtered.is_empty() {
                use std::fmt::Write;
                let mut msg = String::from("No sales found matching criteria");
                if let Some(t) = ticker {
                    let _ = write!(msg, " ticker={t}");
                }
                if let Some(y) = target_year {
                    let _ = write!(msg, " year={y}");
                }
                if let Some(m) = target_month {
                    let _ = write!(msg, " month={m}");
                }
                return Ok(StatResult {
                    mode: StatMode::Empty(msg),
                });
            }

            let title = build_detail_title(ticker, target_year, target_month);
            let mut total_pnl = Decimal::ZERO;
            let rows: Vec<DetailedRow> = filtered
                .iter()
                .map(|e| {
                    total_pnl += e.capital_gain_rsd;
                    DetailedRow {
                        sale_date: e.sale_date.format("%Y-%m-%d").to_string(),
                        quantity: e.quantity,
                        sale_price: e.sale_price,
                        sale_rate: e.sale_exchange_rate,
                        sale_value_rsd: e.sale_value_rsd,
                        buy_date: e.purchase_date.format("%Y-%m-%d").to_string(),
                        buy_price: e.purchase_price,
                        buy_rate: e.purchase_exchange_rate,
                        buy_value_rsd: e.purchase_value_rsd,
                        gain_rsd: e.capital_gain_rsd,
                    }
                })
                .collect();

            Ok(StatResult {
                mode: StatMode::Detailed {
                    rows,
                    total_pnl,
                    title,
                },
            })
        } else {
            let stats = aggregate_stats(
                &sales_entries,
                &transactions,
                self.nbs,
                target_year,
                target_month,
            )?;

            if stats.is_empty() {
                return Ok(StatResult {
                    mode: StatMode::Empty("No data found for the specified filters.".into()),
                });
            }

            Ok(StatResult {
                mode: StatMode::Aggregated(stats),
            })
        }
    }
}

fn parse_month(month: Option<&str>) -> Result<(Option<i32>, Option<u32>)> {
    let Some(s) = month else {
        return Ok((None, None));
    };
    let s = s.trim();

    if s.len() == 7 && s.chars().nth(4) == Some('-') {
        let year: i32 = s[..4]
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid month format: {s}"))?;
        let m: u32 = s[5..]
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid month format: {s}"))?;
        if !(1..=12).contains(&m) {
            bail!("month must be 1-12, got {m}");
        }
        return Ok((Some(year), Some(m)));
    }

    if s.len() == 6 && s.chars().all(|c| c.is_ascii_digit()) {
        let year: i32 = s[..4].parse()?;
        let m: u32 = s[4..].parse()?;
        if !(1..=12).contains(&m) {
            bail!("month must be 1-12, got {m}");
        }
        return Ok((Some(year), Some(m)));
    }

    if s.len() <= 2 && s.chars().all(|c| c.is_ascii_digit()) {
        let m: u32 = s.parse()?;
        if !(1..=12).contains(&m) {
            bail!("month must be 1-12, got {m}");
        }
        return Ok((None, Some(m)));
    }

    bail!("invalid month format: {s} (expected YYYY-MM, YYYYMM, or MM)");
}

fn filter_entries<'a>(
    entries: &'a [TaxReportEntry],
    ticker: Option<&str>,
    year: Option<i32>,
    month: Option<u32>,
) -> Vec<&'a TaxReportEntry> {
    entries
        .iter()
        .filter(|e| {
            if let Some(t) = ticker
                && !e.ticker.eq_ignore_ascii_case(t)
            {
                return false;
            }
            if let Some(y) = year
                && e.sale_date.year() != y
            {
                return false;
            }
            if let Some(m) = month
                && e.sale_date.month() != m
            {
                return false;
            }
            true
        })
        .collect()
}

fn build_detail_title(ticker: Option<&str>, year: Option<i32>, month: Option<u32>) -> String {
    let mut parts = Vec::new();
    if let Some(t) = ticker {
        parts.push(t.to_string());
    }
    if let Some(y) = year {
        if let Some(m) = month {
            parts.push(format!("{y}-{m:02}"));
        } else {
            parts.push(y.to_string());
        }
    }
    format!("Detailed Report: {}", parts.join(" - "))
}

fn aggregate_stats(
    sales_entries: &[TaxReportEntry],
    transactions: &[Transaction],
    nbs: &NBSClient,
    target_year: Option<i32>,
    target_month: Option<u32>,
) -> Result<Vec<AggregatedRow>> {
    // month -> ticker -> {divs, sales_count, pnl}
    let mut stats: BTreeMap<String, BTreeMap<String, (Decimal, usize, Decimal)>> = BTreeMap::new();

    for entry in sales_entries {
        if let Some(y) = target_year
            && entry.sale_date.year() != y
        {
            continue;
        }
        if let Some(m) = target_month
            && entry.sale_date.month() != m
        {
            continue;
        }
        let month_key = entry.sale_date.format("%Y-%m").to_string();
        let bucket = stats
            .entry(month_key)
            .or_default()
            .entry(entry.ticker.clone())
            .or_insert((Decimal::ZERO, 0, Decimal::ZERO));
        bucket.1 += 1;
        bucket.2 += entry.capital_gain_rsd;
    }

    let dividends: Vec<&Transaction> = transactions
        .iter()
        .filter(|t| t.r#type == TransactionType::Dividend)
        .collect();

    for div in &dividends {
        if let Some(y) = target_year
            && div.date.year() != y
        {
            continue;
        }
        if let Some(m) = target_month
            && div.date.month() != m
        {
            continue;
        }

        let amount_rsd = if div.currency == Currency::RSD {
            div.amount
        } else if let Some(rsd) = div.amount_rsd {
            rsd
        } else {
            let rate = nbs.get_rate(div.date, &div.currency)?;
            rate.map_or(div.amount, |r| div.amount * r)
        };

        let month_key = div.date.format("%Y-%m").to_string();
        let bucket = stats
            .entry(month_key)
            .or_default()
            .entry(div.symbol.clone())
            .or_insert((Decimal::ZERO, 0, Decimal::ZERO));
        bucket.0 += amount_rsd;
    }

    let mut rows: Vec<AggregatedRow> = Vec::new();
    for (month, tickers) in &stats {
        for (ticker, (divs, count, pnl)) in tickers {
            rows.push(AggregatedRow {
                month: month.clone(),
                ticker: ticker.clone(),
                dividends_rsd: *divs,
                sales_count: *count,
                realized_pnl_rsd: *pnl,
            });
        }
    }

    rows.sort_by(|a, b| a.ticker.cmp(&b.ticker));
    rows.sort_by(|a, b| b.month.cmp(&a.month));

    Ok(rows)
}

fn find_latest_year_for_month(
    sales: &[TaxReportEntry],
    transactions: &[Transaction],
    month: u32,
) -> Option<i32> {
    let sale_years = sales
        .iter()
        .filter(|e| e.sale_date.month() == month)
        .map(|e| e.sale_date.year());
    let tx_years = transactions
        .iter()
        .filter(|t| t.date.month() == month)
        .map(|t| t.date.year());
    sale_years.chain(tx_years).max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    #[test]
    fn parse_month_yyyy_mm() {
        let (y, m) = parse_month(Some("2025-03")).unwrap();
        assert_eq!(y, Some(2025));
        assert_eq!(m, Some(3));
    }

    #[test]
    fn parse_month_yyyymm() {
        let (y, m) = parse_month(Some("202512")).unwrap();
        assert_eq!(y, Some(2025));
        assert_eq!(m, Some(12));
    }

    #[test]
    fn parse_month_bare_mm() {
        let (y, m) = parse_month(Some("03")).unwrap();
        assert_eq!(y, None);
        assert_eq!(m, Some(3));
    }

    #[test]
    fn parse_month_bare_single_digit() {
        let (y, m) = parse_month(Some("9")).unwrap();
        assert_eq!(y, None);
        assert_eq!(m, Some(9));
    }

    #[test]
    fn parse_month_none() {
        let (y, m) = parse_month(None).unwrap();
        assert_eq!(y, None);
        assert_eq!(m, None);
    }

    #[test]
    fn parse_month_invalid_month_number() {
        assert!(parse_month(Some("2025-13")).is_err());
        assert!(parse_month(Some("2025-00")).is_err());
        assert!(parse_month(Some("13")).is_err());
    }

    #[test]
    fn parse_month_invalid_format() {
        assert!(parse_month(Some("abc")).is_err());
        assert!(parse_month(Some("2025-1-1")).is_err());
    }

    #[test]
    fn parse_month_yyyymm_invalid() {
        assert!(parse_month(Some("202513")).is_err());
        assert!(parse_month(Some("202500")).is_err());
    }

    #[test]
    fn parse_month_bare_zero() {
        assert!(parse_month(Some("0")).is_err());
    }

    fn make_entry(ticker: &str, sale_date: NaiveDate) -> TaxReportEntry {
        TaxReportEntry {
            ticker: ticker.into(),
            quantity: Decimal::ONE,
            sale_date,
            sale_price: Decimal::ONE,
            sale_exchange_rate: Decimal::ONE,
            sale_value_rsd: Decimal::ONE,
            purchase_date: sale_date,
            purchase_price: Decimal::ONE,
            purchase_exchange_rate: Decimal::ONE,
            purchase_value_rsd: Decimal::ONE,
            capital_gain_rsd: Decimal::ONE,
            is_tax_exempt: false,
        }
    }

    #[test]
    fn filter_entries_by_ticker() {
        let entries = vec![
            make_entry("AAPL", NaiveDate::from_ymd_opt(2025, 3, 1).unwrap()),
            make_entry("MSFT", NaiveDate::from_ymd_opt(2025, 3, 1).unwrap()),
        ];
        let result = filter_entries(&entries, Some("AAPL"), None, None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ticker, "AAPL");
    }

    #[test]
    fn filter_entries_by_year_and_month() {
        let entries = vec![
            make_entry("AAPL", NaiveDate::from_ymd_opt(2025, 3, 1).unwrap()),
            make_entry("AAPL", NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()),
            make_entry("AAPL", NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()),
        ];
        let result = filter_entries(&entries, None, Some(2025), Some(3));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].sale_date.year(), 2025);
        assert_eq!(result[0].sale_date.month(), 3);
    }

    #[test]
    fn filter_entries_empty_on_no_match() {
        let entries = vec![make_entry(
            "AAPL",
            NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
        )];
        let result = filter_entries(&entries, Some("GOOG"), None, None);
        assert!(result.is_empty());
    }

    #[test]
    fn build_detail_title_ticker_only() {
        let title = build_detail_title(Some("AAPL"), None, None);
        assert!(title.contains("AAPL"));
    }

    #[test]
    fn build_detail_title_year_month() {
        let title = build_detail_title(Some("AAPL"), Some(2025), Some(3));
        assert!(title.contains("AAPL"));
        assert!(title.contains("2025-03"));
    }

    #[test]
    fn build_detail_title_year_only() {
        let title = build_detail_title(None, Some(2025), None);
        assert!(title.contains("2025"));
    }

    #[test]
    fn find_latest_year_for_month_from_sales() {
        let entries = vec![
            make_entry("A", NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()),
            make_entry("A", NaiveDate::from_ymd_opt(2025, 3, 1).unwrap()),
        ];
        let result = find_latest_year_for_month(&entries, &[], 3);
        assert_eq!(result, Some(2025));
    }

    #[test]
    fn find_latest_year_for_month_none() {
        let result = find_latest_year_for_month(&[], &[], 7);
        assert_eq!(result, None);
    }
}
