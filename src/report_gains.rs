use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

use crate::declaration_gains_xml::generate_gains_xml;
use crate::holidays::HolidayCalendar;
use crate::models::{
    CarryforwardSource, CarryforwardVintage, TaxReportEntry, UserConfig, sort_oldest_origin_first,
};
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
    fn gross_and_losses(&self) -> (Decimal, Decimal) {
        let gross = self
            .entries
            .iter()
            .map(|e| e.capital_gain_rsd.max(Decimal::ZERO))
            .sum();
        let losses = self
            .entries
            .iter()
            .map(|e| e.capital_gain_rsd.min(Decimal::ZERO).abs())
            .sum();
        (gross, losses)
    }

    /// Calculated tax base for the period: gross gains minus losses, floored at zero.
    #[must_use]
    pub fn tax_base(&self) -> Decimal {
        let (gross, losses) = self.gross_and_losses();
        (gross - losses).max(Decimal::ZERO)
    }

    /// Summary metadata for declaration storage.
    #[must_use]
    pub fn metadata(&self) -> indexmap::IndexMap<String, serde_json::Value> {
        let total_gain: Decimal = self.entries.iter().map(|e| e.capital_gain_rsd).sum();
        let (gross, losses) = self.gross_and_losses();
        let tax_base = (gross - losses).max(Decimal::ZERO);
        let tax = (tax_base * Decimal::new(15, 2))
            .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

        let mut m = indexmap::IndexMap::new();
        m.insert("entry_count".into(), self.entries.len().into());
        m.insert("period_start".into(), fmt_date(self.period_start).into());
        m.insert("period_end".into(), fmt_date(self.period_end).into());
        m.insert("total_gain_rsd".into(), format!("{total_gain:.2}").into());
        m.insert("gross_income_rsd".into(), format!("{gross:.2}").into());
        m.insert("tax_base_rsd".into(), format!("{tax_base:.2}").into());
        m.insert("calculated_tax_rsd".into(), format!("{tax:.2}").into());
        m.insert("estimated_tax_rsd".into(), format!("{tax:.2}").into());
        m.insert("foreign_tax_paid_rsd".into(), "0.00".into());
        m
    }
}

/// Result of applying eligible carryforward vintages against a calculated tax base.
pub struct CarryforwardApplication {
    pub opening_carryforward_rsd: Decimal,
    pub carryforward_used_rsd: Decimal,
    pub closing_carryforward_rsd: Decimal,
    pub adjusted_tax_base_rsd: Decimal,
    pub estimated_tax_rsd: Decimal,
    pub sources: Vec<CarryforwardSource>,
}

impl CarryforwardApplication {
    /// Merge this application's results into a declaration metadata map.
    pub fn apply_to_metadata(&self, m: &mut indexmap::IndexMap<String, serde_json::Value>) {
        m.insert(
            "opening_carryforward_rsd".into(),
            format!("{:.2}", self.opening_carryforward_rsd).into(),
        );
        m.insert(
            "carryforward_used_rsd".into(),
            format!("{:.2}", self.carryforward_used_rsd).into(),
        );
        m.insert(
            "closing_carryforward_rsd".into(),
            format!("{:.2}", self.closing_carryforward_rsd).into(),
        );
        m.insert(
            "adjusted_tax_base_rsd".into(),
            format!("{:.2}", self.adjusted_tax_base_rsd).into(),
        );
        m.insert(
            "estimated_tax_rsd".into(),
            format!("{:.2}", self.estimated_tax_rsd).into(),
        );
        let sources: Vec<serde_json::Value> = self
            .sources
            .iter()
            .map(|s| {
                serde_json::json!({
                    "vintage_id": s.vintage_id,
                    "amount_used": format!("{:.2}", s.amount_used),
                })
            })
            .collect();
        m.insert(
            "carryforward_sources".into(),
            serde_json::Value::Array(sources),
        );
    }
}

/// Read-only: does not mutate the ledger. `current_tax_year` is the tax year
/// of the declaration being generated (`period_end.year()`).
#[must_use]
pub fn compute_carryforward_application(
    storage: &Storage,
    calculated_tax_base: Decimal,
    current_tax_year: i32,
) -> CarryforwardApplication {
    let mut eligible: Vec<CarryforwardVintage> = storage
        .get_carryforward_vintages()
        .into_iter()
        .filter(|v| v.is_eligible(current_tax_year))
        .collect();
    sort_oldest_origin_first(&mut eligible);

    let opening: Decimal = eligible.iter().map(|v| v.remaining_loss_rsd).sum();

    let mut remaining_base = calculated_tax_base.max(Decimal::ZERO);
    let mut sources = Vec::new();
    let mut used = Decimal::ZERO;
    for v in &eligible {
        if remaining_base <= Decimal::ZERO {
            break;
        }
        let take = v.remaining_loss_rsd.min(remaining_base);
        if take > Decimal::ZERO {
            sources.push(CarryforwardSource {
                vintage_id: v.id.clone(),
                amount_used: take,
            });
            used += take;
            remaining_base -= take;
        }
    }

    let adjusted_tax_base = (calculated_tax_base - used).max(Decimal::ZERO);
    let estimated_tax = (adjusted_tax_base * Decimal::new(15, 2))
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);

    CarryforwardApplication {
        opening_carryforward_rsd: opening,
        carryforward_used_rsd: used,
        closing_carryforward_rsd: opening - used,
        adjusted_tax_base_rsd: adjusted_tax_base,
        estimated_tax_rsd: estimated_tax,
        sources,
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
