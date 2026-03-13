use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

use crate::declaration_gains_xml::generate_gains_xml;
use crate::holidays::HolidayCalendar;
use crate::models::{TaxReportEntry, UserConfig};
use crate::nbs::NBSClient;
use crate::storage::Storage;
use crate::tax::TaxCalculator;

pub struct GainsReport {
    pub filename: String,
    pub xml_content: String,
    pub entries: Vec<TaxReportEntry>,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}

impl GainsReport {
    /// Summary metadata for declaration storage.
    #[must_use]
    pub fn metadata(&self) -> indexmap::IndexMap<String, serde_json::Value> {
        let total_gain: Decimal = self.entries.iter().map(|e| e.capital_gain_rsd).sum();
        let gross: Decimal = self
            .entries
            .iter()
            .map(|e| e.capital_gain_rsd.max(Decimal::ZERO))
            .sum();
        let losses: Decimal = self
            .entries
            .iter()
            .map(|e| e.capital_gain_rsd.min(Decimal::ZERO).abs())
            .sum();
        let tax_base = (gross - losses).max(Decimal::ZERO);
        let tax = (tax_base * Decimal::new(15, 2))
            .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        let mut m = indexmap::IndexMap::new();
        m.insert("entry_count".into(), self.entries.len().into());
        m.insert("period_start".into(), fmt_date(self.period_start).into());
        m.insert("period_end".into(), fmt_date(self.period_end).into());
        m.insert("total_gain_rsd".into(), total_gain.to_string().into());
        m.insert("gross_income_rsd".into(), gross.to_string().into());
        m.insert("tax_base_rsd".into(), tax_base.to_string().into());
        m.insert("calculated_tax_rsd".into(), tax.to_string().into());
        m.insert("estimated_tax_rsd".into(), tax.to_string().into());
        m.insert("foreign_tax_paid_rsd".into(), "0.00".into());
        m
    }
}

/// Generate a PPDG-3R capital gains report for the given half-year period.
///
/// Loads **all** transactions to build the FIFO chain, then filters entries
/// whose `sale_date` falls within `[period_start, period_end]`.
pub fn generate_gains_report(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    period_start: NaiveDate,
    period_end: NaiveDate,
    force: bool,
) -> Result<GainsReport> {
    let transactions = storage.load_transactions();
    let calc = TaxCalculator::with_force(nbs, force);
    let all_entries = calc.process_trades(&transactions)?;

    let entries: Vec<TaxReportEntry> = all_entries
        .into_iter()
        .filter(|e| e.sale_date >= period_start && e.sale_date <= period_end)
        .collect();

    if entries.is_empty() {
        bail!("no taxable sales in period {period_start} to {period_end}");
    }

    let half = if period_end.month() <= 6 { 1 } else { 2 };
    let year = period_end.year();
    let filename = format!("ppdg3r-{year}-H{half}.xml");

    let xml = generate_gains_xml(&entries, config, period_end, holidays);

    Ok(GainsReport {
        filename,
        xml_content: xml,
        entries,
        period_start,
        period_end,
    })
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}
