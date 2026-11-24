use std::str::FromStr;

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use rust_decimal::Decimal;

use crate::models::{CarryforwardLedger, CarryforwardSource, Declaration, ReflectedDeductions};

pub const ORIGINAL_SOURCES_KEY: &str = "carryforward_sources";
pub const ASSESSED_SOURCES_KEY: &str = "assessed_carryforward_sources";

fn parse_sources(decl: &Declaration, key: &str) -> Result<Option<Vec<CarryforwardSource>>> {
    let Some(value) = decl.metadata.get(key) else {
        return Ok(None);
    };
    let Some(arr) = value.as_array() else {
        bail!(
            "declaration {} has malformed {key} metadata (expected an array)",
            decl.declaration_id
        );
    };
    arr.iter()
        .map(|s| {
            let vintage_id = s
                .get("vintage_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("{key} entry missing vintage_id"))?
                .to_string();
            let amount_used = s
                .get("amount_used")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("{key} entry missing amount_used"))?
                .parse::<Decimal>()
                .with_context(|| format!("{key} amount_used is not a decimal"))?;
            Ok(CarryforwardSource {
                vintage_id,
                amount_used,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

#[must_use]
pub fn sources_to_json(sources: &[CarryforwardSource]) -> serde_json::Value {
    serde_json::Value::Array(
        sources
            .iter()
            .map(|s| {
                serde_json::json!({
                    "vintage_id": s.vintage_id,
                    "amount_used": format!("{:.2}", s.amount_used),
                })
            })
            .collect(),
    )
}

/// Prior-loss deductions the declaration made when it was generated.
pub fn original_deductions(decl: &Declaration) -> Result<Vec<CarryforwardSource>> {
    Ok(parse_sources(decl, ORIGINAL_SOURCES_KEY)?.unwrap_or_default())
}

/// Prior-loss use established by a tax-authority assessment, if recorded.
pub fn assessed_deductions(decl: &Declaration) -> Result<Option<Vec<CarryforwardSource>>> {
    parse_sources(decl, ASSESSED_SOURCES_KEY)
}

fn ledger_record<'a>(
    ledger: &'a CarryforwardLedger,
    decl: &Declaration,
) -> Option<&'a ReflectedDeductions> {
    ledger.reflected_deductions.iter().find(|r| {
        r.declaration_id == decl.declaration_id && r.declaration_created_at == decl.created_at
    })
}

/// Deductions of `decl` currently reflected in the ledger balances. The
/// ledger's own record wins over declaration metadata because it is written
/// in the same file write as the balances it describes.
pub fn reflected_deductions(
    ledger: &CarryforwardLedger,
    decl: &Declaration,
) -> Result<Vec<CarryforwardSource>> {
    if let Some(record) = ledger_record(ledger, decl) {
        return Ok(record.sources.clone());
    }
    if let Some(assessed) = assessed_deductions(decl)? {
        return Ok(assessed);
    }
    original_deductions(decl)
}

fn totals_by_vintage(sources: &[CarryforwardSource]) -> IndexMap<String, Decimal> {
    let mut totals: IndexMap<String, Decimal> = IndexMap::new();
    for s in sources {
        *totals.entry(s.vintage_id.clone()).or_default() += s.amount_used;
    }
    totals
}

/// Bring the ledger from the deductions it currently reflects for `decl` to
/// `target`, applying only the per-vintage difference, and record `target`
/// as reflected. In-memory only; the caller persists the ledger.
pub fn reconcile(
    ledger: &mut CarryforwardLedger,
    decl: &Declaration,
    target: &[CarryforwardSource],
) -> Result<()> {
    let reflected = reflected_deductions(ledger, decl)?;
    let reflected_totals = totals_by_vintage(&reflected);
    let target_totals = totals_by_vintage(target);
    if reflected_totals == target_totals {
        return Ok(());
    }

    let mut deltas: IndexMap<String, Decimal> = IndexMap::new();
    for (id, amount) in &target_totals {
        *deltas.entry(id.clone()).or_default() += *amount;
    }
    for (id, amount) in &reflected_totals {
        *deltas.entry(id.clone()).or_default() -= *amount;
    }

    for (vintage_id, delta) in &deltas {
        if delta.is_zero() {
            continue;
        }
        let Some(v) = ledger.vintages.iter().find(|v| &v.id == vintage_id) else {
            bail!(
                "carryforward vintage {vintage_id} used by declaration {} is missing from the ledger",
                decl.declaration_id
            );
        };
        let new_remaining = v.remaining_loss_rsd - delta;
        if new_remaining < Decimal::ZERO || new_remaining > v.recognized_loss_rsd {
            bail!(
                "cannot adjust carryforward vintage {vintage_id} by {:.2} RSD for declaration {}: \
                 remaining balance {:.2} of {:.2} RSD recognized",
                -delta,
                decl.declaration_id,
                v.remaining_loss_rsd,
                v.recognized_loss_rsd
            );
        }
    }

    for v in &mut ledger.vintages {
        if let Some(delta) = deltas.get(&v.id) {
            v.remaining_loss_rsd -= *delta;
        }
    }

    let record = ReflectedDeductions {
        declaration_id: decl.declaration_id.clone(),
        declaration_created_at: decl.created_at,
        sources: target.to_vec(),
    };
    if let Some(existing) = ledger.reflected_deductions.iter_mut().find(|r| {
        r.declaration_id == decl.declaration_id && r.declaration_created_at == decl.created_at
    }) {
        *existing = record;
    } else {
        ledger.reflected_deductions.push(record);
    }
    Ok(())
}

/// Drop the ledger's record for a declaration that no longer exists.
pub fn forget(ledger: &mut CarryforwardLedger, decl: &Declaration) {
    ledger.reflected_deductions.retain(|r| {
        !(r.declaration_id == decl.declaration_id && r.declaration_created_at == decl.created_at)
    });
}

fn metadata_decimal(decl: &Declaration, key: &str) -> Option<Decimal> {
    decl.metadata
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| Decimal::from_str(s).ok())
}

/// Prior-loss use the recorded assessment establishes: none when the period
/// itself has no gain to offset (recognized gain of zero, or a recognized
/// loss). A positive recognized gain or a tax amount alone cannot tell how
/// much prior loss was applied, so they establish nothing.
#[must_use]
pub fn established_by_assessment(decl: &Declaration) -> Option<Vec<CarryforwardSource>> {
    let gain = metadata_decimal(decl, "recognized_capital_gain_rsd");
    let loss = metadata_decimal(decl, "recognized_capital_loss_rsd");
    let no_gain = gain.is_some_and(|g| g.is_zero()) || loss.is_some_and(|l| l > Decimal::ZERO);
    no_gain.then(Vec::new)
}
