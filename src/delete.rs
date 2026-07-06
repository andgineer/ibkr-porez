use anyhow::{Context, Result, bail};
use rust_decimal::Decimal;

use crate::config;
use crate::models::{CarryforwardSource, Declaration, DeclarationType, UserConfig};
use crate::storage::Storage;

#[derive(Debug)]
pub struct DeletePlan {
    pub to_delete: Declaration,
    /// `CF-{id}` exists and will be removed from the ledger.
    pub deletes_vintage: bool,
}

/// Build a deletion plan without mutating anything. Validates that the
/// declaration exists and that deleting it keeps the carryforward ledger
/// consistent (see the guards).
pub fn plan_deletion(storage: &Storage, declaration_id: &str) -> Result<DeletePlan> {
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
                "later PPDG-3R declarations exist; delete the newer ones first (deleting {declaration_id} would leave their carryforward dangling)"
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

    Ok(DeletePlan {
        to_delete,
        deletes_vintage,
    })
}

/// Execute the plan: undo the target's ledger effects, then remove it (records
/// and files). The declaration's period is *not* regenerated — run `sync` to
/// rebuild it if needed.
pub fn execute_deletion(storage: &Storage, config: &UserConfig, plan: &DeletePlan) -> Result<()> {
    let decl = &plan.to_delete;

    // 1. Reverse the carryforward this declaration consumed from other vintages.
    let reversal = reversed_consumption(decl)?;
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

    Ok(())
}

/// Parse the target's `carryforward_sources` metadata and negate each
/// `amount_used`, producing the reversal to feed to
/// `apply_carryforward_consumption`. Absent metadata yields no reversal;
/// malformed metadata bails before any ledger mutation, since silently
/// dropping an entry would leave the ledger under-restored.
fn reversed_consumption(decl: &Declaration) -> Result<Vec<CarryforwardSource>> {
    let Some(sources) = decl.metadata.get("carryforward_sources") else {
        return Ok(Vec::new());
    };
    let Some(arr) = sources.as_array() else {
        bail!(
            "declaration {} has malformed carryforward_sources metadata (expected an array)",
            decl.declaration_id
        );
    };
    arr.iter()
        .map(|s| {
            let vintage_id = s
                .get("vintage_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("carryforward_sources entry missing vintage_id"))?
                .to_string();
            let amount = s
                .get("amount_used")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("carryforward_sources entry missing amount_used"))?
                .parse::<Decimal>()
                .with_context(|| "carryforward_sources amount_used is not a decimal")?;
            Ok(CarryforwardSource {
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
