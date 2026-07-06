use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use tracing::debug;

use crate::config;
use crate::holidays::HolidayCalendar;
use crate::models::{CarryforwardSource, Declaration, DeclarationType, UserConfig};
use crate::nbs::NBSClient;
use crate::storage::Storage;
use crate::sync::{generate_and_save_gains_for_period, generate_and_save_income_for_period};

#[derive(Debug)]
pub struct RegenerationPlan {
    pub to_delete: Declaration,
    pub period_to_generate: (NaiveDate, NaiveDate),
    /// `CF-{id}` exists and will be removed from the ledger.
    pub deletes_vintage: bool,
}

/// Build a regeneration plan without mutating anything. Validates that the
/// declaration exists and that regenerating it is safe (see the guards).
pub fn plan_regeneration(storage: &Storage, declaration_id: &str) -> Result<RegenerationPlan> {
    let to_delete = storage
        .get_declaration(declaration_id)
        .ok_or_else(|| anyhow::anyhow!("declaration {declaration_id} not found"))?;

    if to_delete.r#type == DeclarationType::Ppdg3r {
        let has_later = storage
            .get_declarations(None, Some(&DeclarationType::Ppdg3r))
            .iter()
            .any(|d| {
                d.declaration_id != to_delete.declaration_id
                    && d.period_start > to_delete.period_start
            });
        if has_later {
            bail!(
                "later PPDG-3R declarations exist; regenerating {declaration_id} would invalidate them — not supported"
            );
        }
    }

    let vintage_id = format!("CF-{declaration_id}");
    let deletes_vintage = match storage.find_carryforward_vintage(&vintage_id) {
        Some(v) => {
            if v.remaining_loss_rsd != v.recognized_loss_rsd {
                bail!(
                    "carryforward vintage {vintage_id} has been consumed; fix the ledger manually first"
                );
            }
            true
        }
        None => false,
    };

    Ok(RegenerationPlan {
        period_to_generate: (to_delete.period_start, to_delete.period_end),
        to_delete,
        deletes_vintage,
    })
}

/// Execute the plan: undo the target's ledger effects, delete it (records and
/// files), then regenerate its period from stored transactions. Returns the
/// newly created declarations.
pub fn execute_regeneration(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    plan: &RegenerationPlan,
    force: bool,
) -> Result<Vec<Declaration>> {
    let decl = &plan.to_delete;

    // 1. Reverse the carryforward this declaration consumed from other vintages.
    let reversal = reversed_consumption(decl);
    storage
        .apply_carryforward_consumption(&reversal)
        .with_context(|| storage.io_error_hint())?;

    // 2. Remove the declaration's own vintage (created by `assess`). Re-check
    //    the guard: since no later PPDG-3R exists it cannot have been consumed,
    //    but a manually edited ledger might disagree — bail before mutating.
    let vintage_id = format!("CF-{}", decl.declaration_id);
    if let Some(v) = storage.find_carryforward_vintage(&vintage_id) {
        if v.remaining_loss_rsd != v.recognized_loss_rsd {
            bail!(
                "carryforward vintage {vintage_id} has been consumed; fix the ledger manually first"
            );
        }
        storage
            .remove_carryforward_vintage(&vintage_id)
            .with_context(|| storage.io_error_hint())?;
    }

    // 3. Delete files (best-effort — missing files are not an error).
    delete_declaration_files(storage, config, decl);

    // 4. Delete the declaration record.
    storage.delete_declaration(&decl.declaration_id)?;

    // 5. Regenerate the period from stored transactions.
    let output_dir = config::get_effective_output_dir_path(config);
    std::fs::create_dir_all(&output_dir)?;
    let (start, end) = plan.period_to_generate;

    match decl.r#type {
        DeclarationType::Ppdg3r => {
            match generate_and_save_gains_for_period(
                storage,
                nbs,
                config,
                holidays,
                start,
                end,
                &output_dir,
                force,
            ) {
                Ok(decls) => Ok(decls),
                Err(e) => {
                    // After the bug fix the period may genuinely have nothing to
                    // declare; treat that as a clean rebuild with no output.
                    if e.to_string().contains("no taxable sales") {
                        debug!("no taxable sales in regenerated period, nothing to declare");
                        Ok(Vec::new())
                    } else {
                        Err(e)
                    }
                }
            }
        }
        DeclarationType::Ppo => generate_and_save_income_for_period(
            storage,
            nbs,
            config,
            holidays,
            start,
            end,
            &output_dir,
            force,
        ),
    }
}

/// Parse the target's `carryforward_sources` metadata and negate each
/// `amount_used`, producing the reversal to feed to
/// `apply_carryforward_consumption`. Absent/empty metadata yields no reversal.
fn reversed_consumption(decl: &Declaration) -> Vec<CarryforwardSource> {
    let Some(sources) = decl.metadata.get("carryforward_sources") else {
        return Vec::new();
    };
    let Some(arr) = sources.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|s| {
            let vintage_id = s.get("vintage_id")?.as_str()?.to_string();
            let amount = s.get("amount_used")?.as_str()?.parse::<Decimal>().ok()?;
            Some(CarryforwardSource {
                vintage_id,
                amount_used: -amount,
            })
        })
        .collect()
}

fn delete_declaration_files(storage: &Storage, config: &UserConfig, decl: &Declaration) {
    if let Some(ref path) = decl.file_path {
        let xml_path = std::path::Path::new(path);
        let _ = std::fs::remove_file(xml_path);

        if let Some(name) = xml_path.file_name() {
            let output_copy = config::get_effective_output_dir_path(config).join(name);
            let _ = std::fs::remove_file(output_copy);
        }
    }

    let attachments = storage.declarations_dir().join(&decl.declaration_id);
    let _ = std::fs::remove_dir_all(attachments);
}
