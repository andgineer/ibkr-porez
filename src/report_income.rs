use std::collections::BTreeMap;
use std::sync::LazyLock;

use anyhow::{Result, bail};
use chrono::{Duration, NaiveDate};
use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use tracing::debug;

use crate::declaration_income_xml::generate_income_xml;
use crate::holidays::HolidayCalendar;
use crate::models::{
    Currency, INCOME_CODE_COUPON, INCOME_CODE_DIVIDEND, IncomeDeclarationEntry, IncomeEntry,
    Transaction, TransactionType, UserConfig,
};
use crate::nbs::NBSClient;
use crate::storage::Storage;

type GroupKey = (NaiveDate, String, String);

static ENTITY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9A-Za-z.]+)\s*\(([0-9A-Za-z]+)\)").unwrap());

pub struct IncomeReport {
    pub filename: String,
    pub xml_content: String,
    pub entries: Vec<IncomeDeclarationEntry>,
    pub declaration_date: NaiveDate,
}

impl IncomeReport {
    #[must_use]
    pub fn metadata(&self) -> indexmap::IndexMap<String, serde_json::Value> {
        let mut m = indexmap::IndexMap::new();
        m.insert("entry_count".into(), self.entries.len().into());

        let income_type = if self
            .entries
            .first()
            .is_some_and(|e| e.sifra_vrste_prihoda == INCOME_CODE_DIVIDEND)
        {
            "dividend"
        } else {
            "coupon"
        };
        m.insert("income_type".into(), income_type.into());

        let symbol = self
            .entries
            .first()
            .and_then(|e| e.symbol_or_currency.as_deref())
            .unwrap_or("")
            .to_uppercase();
        m.insert("symbol".into(), symbol.into());

        let dates: Vec<NaiveDate> = self.entries.iter().map(|e| e.date).collect();
        if let (Some(min), Some(max)) = (dates.iter().min(), dates.iter().max()) {
            m.insert("period_start".into(), fmt_date(*min).into());
            m.insert("period_end".into(), fmt_date(*max).into());
        }

        let gross: Decimal = self.entries.iter().map(|e| e.bruto_prihod).sum();
        let tax_base: Decimal = self.entries.iter().map(|e| e.osnovica_za_porez).sum();
        let calc_tax: Decimal = self.entries.iter().map(|e| e.obracunati_porez).sum();
        let foreign: Decimal = self
            .entries
            .iter()
            .map(|e| e.porez_placen_drugoj_drzavi)
            .sum();
        let due: Decimal = self.entries.iter().map(|e| e.porez_za_uplatu).sum();

        m.insert("gross_income_rsd".into(), format!("{gross:.2}").into());
        m.insert("tax_base_rsd".into(), format!("{tax_base:.2}").into());
        m.insert("calculated_tax_rsd".into(), format!("{calc_tax:.2}").into());
        m.insert(
            "foreign_tax_paid_rsd".into(),
            format!("{foreign:.2}").into(),
        );
        m.insert("tax_due_rsd".into(), format!("{due:.2}").into());
        m
    }
}

/// Generate PP-OPO income declarations for dividends and interest in the given period.
pub fn generate_income_reports(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    start: NaiveDate,
    end: NaiveDate,
    force: bool,
) -> Result<Vec<IncomeReport>> {
    let all_txns = storage.load_transactions();

    let income_txns: Vec<&Transaction> = all_txns
        .iter()
        .filter(|t| {
            (t.r#type == TransactionType::Dividend || t.r#type == TransactionType::Interest)
                && t.date >= start
                && t.date <= end
        })
        .collect();

    if income_txns.is_empty() {
        return Ok(Vec::new());
    }

    let wht_pool: Vec<&Transaction> = all_txns
        .iter()
        .filter(|t| {
            t.r#type == TransactionType::WithholdingTax
                && t.date >= start
                && t.date <= end + Duration::days(7)
        })
        .collect();

    let groups = build_income_groups(&income_txns, &wht_pool, nbs, force)?;
    build_income_reports(&groups, config, holidays, force)
}

fn build_income_groups<'a>(
    income_txns: &[&'a Transaction],
    wht_pool: &[&'a Transaction],
    nbs: &NBSClient,
    force: bool,
) -> Result<BTreeMap<GroupKey, Vec<IncomeEntry>>> {
    let mut groups: BTreeMap<GroupKey, Vec<IncomeEntry>> = BTreeMap::new();

    for txn in income_txns {
        let income_type = match txn.r#type {
            TransactionType::Dividend => "dividend",
            TransactionType::Interest => "coupon",
            _ => continue,
        };

        let rate = get_rate_or_force(nbs, txn.date, &txn.currency, force)?;
        let amount_rsd = round2(txn.amount.abs() * rate);

        let wht_rsd = find_withholding_tax(wht_pool, txn, income_type, nbs, force)?;

        let group_key_symbol = if income_type == "dividend" {
            txn.symbol.clone()
        } else {
            txn.currency.as_code().to_string()
        };

        let ie = IncomeEntry {
            date: txn.date,
            symbol: txn.symbol.clone(),
            amount: txn.amount.abs(),
            currency: txn.currency.clone(),
            amount_rsd,
            exchange_rate: round4(rate),
            income_type: income_type.to_string(),
            description: txn.description.clone(),
            withholding_tax_usd: Decimal::ZERO,
            withholding_tax_rsd: wht_rsd,
        };

        let key = (txn.date, group_key_symbol, income_type.to_string());
        groups.entry(key).or_default().push(ie);
    }

    Ok(groups)
}

fn build_income_reports(
    groups: &BTreeMap<GroupKey, Vec<IncomeEntry>>,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    force: bool,
) -> Result<Vec<IncomeReport>> {
    let mut reports = Vec::new();

    for ((date, sym_or_cur, income_type), group) in groups {
        let total_bruto: Decimal = round2(group.iter().map(|e| e.amount_rsd).sum());
        let total_wht_rsd: Decimal = round2(group.iter().map(|e| e.withholding_tax_rsd).sum());

        if total_wht_rsd == Decimal::ZERO && !force {
            bail!(
                "zero withholding tax for {sym_or_cur} on {date} -- \
                 tax may arrive later; use --force to override"
            );
        }

        let sifra = if income_type == "dividend" {
            INCOME_CODE_DIVIDEND
        } else {
            INCOME_CODE_COUPON
        };

        let osnovica = total_bruto;
        let obracunati = round2_half_up(osnovica * Decimal::new(15, 2));
        let porez_placen = total_wht_rsd;
        let porez_za_uplatu = round2((obracunati - porez_placen).max(Decimal::ZERO));

        let decl_entry = IncomeDeclarationEntry {
            date: *date,
            symbol_or_currency: Some(sym_or_cur.clone()),
            sifra_vrste_prihoda: sifra.to_string(),
            bruto_prihod: total_bruto,
            osnovica_za_porez: osnovica,
            obracunati_porez: obracunati,
            porez_placen_drugoj_drzavi: porez_placen,
            porez_za_uplatu,
        };

        let xml = generate_income_xml(&decl_entry, config, holidays);

        let key_lower = sym_or_cur.to_lowercase();
        let date_part = date.format("%Y-%m%d").to_string();
        let filename = format!("ppopo-{key_lower}-{date_part}.xml");

        debug!(%filename, %total_bruto, %porez_za_uplatu, "generated income declaration");

        reports.push(IncomeReport {
            filename,
            xml_content: xml,
            entries: vec![decl_entry],
            declaration_date: *date,
        });
    }

    Ok(reports)
}

fn find_withholding_tax(
    wht_pool: &[&Transaction],
    income_txn: &Transaction,
    income_type: &str,
    nbs: &NBSClient,
    force: bool,
) -> Result<Decimal> {
    let window_start = income_txn.date;
    let window_end = income_txn.date + Duration::days(7);

    let candidates: Vec<&&Transaction> = wht_pool
        .iter()
        .filter(|wht| wht.date >= window_start && wht.date <= window_end)
        .collect();

    let mut matched_rsd = Decimal::ZERO;

    if income_type == "dividend" {
        if let Some(caps) = ENTITY_RE.captures(&income_txn.description) {
            let entity_name = caps.get(1).map_or("", |m| m.as_str());
            let entity_isin = caps.get(2).map_or("", |m| m.as_str());

            for wht in &candidates {
                let desc = &wht.description;
                if desc.contains(entity_name) || desc.contains(entity_isin) {
                    let rate = get_rate_or_force(nbs, wht.date, &wht.currency, force)?;
                    matched_rsd += round2(wht.amount.abs() * rate);
                }
            }
        }

        if matched_rsd == Decimal::ZERO {
            for wht in &candidates {
                if wht.symbol == income_txn.symbol {
                    let rate = get_rate_or_force(nbs, wht.date, &wht.currency, force)?;
                    matched_rsd += round2(wht.amount.abs() * rate);
                }
            }
        }
    } else {
        for wht in &candidates {
            if wht.currency == income_txn.currency {
                let rate = get_rate_or_force(nbs, wht.date, &wht.currency, force)?;
                matched_rsd += round2(wht.amount.abs() * rate);
            }
        }
    }

    Ok(matched_rsd)
}

const FORCE_LOOKBACK_DAYS: u32 = 365;

fn get_rate_or_force(
    nbs: &NBSClient,
    date: NaiveDate,
    currency: &Currency,
    force: bool,
) -> Result<Decimal> {
    if *currency == Currency::RSD {
        return Ok(Decimal::ONE);
    }
    match nbs.get_rate(date, currency) {
        Ok(Some(rate)) => Ok(rate),
        Ok(None) if force => force_fallback_rate(nbs, date, currency),
        Ok(None) => Err(anyhow::anyhow!(
            "no NBS exchange rate for {currency:?} on {date}"
        )),
        Err(e) if force => {
            tracing::warn!(%date, ?currency, error = %e, "rate error in force mode, trying cache");
            force_fallback_rate(nbs, date, currency)
        }
        Err(e) => Err(e),
    }
}

fn force_fallback_rate(nbs: &NBSClient, date: NaiveDate, currency: &Currency) -> Result<Decimal> {
    if let Some(cached) =
        nbs.storage()
            .find_nearest_cached_rate(date, currency, FORCE_LOOKBACK_DAYS)
    {
        tracing::warn!(
            %date, ?currency, fallback_date = %cached.date, rate = %cached.rate,
            "force mode: using nearest cached rate"
        );
        Ok(cached.rate)
    } else {
        tracing::warn!(%date, ?currency, "force mode: no cached rate found at all");
        Err(anyhow::anyhow!(
            "no NBS exchange rate found for {currency:?} on {date} (even with force lookback)"
        ))
    }
}

fn round2(d: Decimal) -> Decimal {
    d.round_dp(2)
}

fn round4(d: Decimal) -> Decimal {
    d.round_dp(4)
}

fn round2_half_up(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}
