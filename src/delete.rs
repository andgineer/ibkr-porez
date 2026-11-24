use anyhow::{Context, Result, bail};

use crate::config;
use crate::models::{Declaration, DeclarationType, UserConfig};
use crate::prior_loss_use;
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

    // 1. In one ledger write: return the prior-loss deductions the ledger still
    //    reflects for this declaration, and remove its own vintage (created by
    //    `assess`). The own-vintage guard is re-checked because a manually
    //    edited ledger might disagree with the plan.
    let mut ledger = storage.load_carryforward_ledger();
    let before = ledger.clone();
    prior_loss_use::reconcile(&mut ledger, decl, &[])?;
    let vintage_id = format!("CF-{}", decl.declaration_id);
    if let Some(v) = ledger.vintages.iter().find(|v| v.id == vintage_id) {
        if v.remaining_loss_rsd != v.recognized_loss_rsd {
            bail!(
                "carryforward vintage {vintage_id} has been consumed; fix the ledger manually first"
            );
        }
        ledger.vintages.retain(|v| v.id != vintage_id);
    }
    if ledger != before {
        storage
            .save_carryforward_ledger(&ledger)
            .with_context(|| storage.io_error_hint())?;
    }

    // 2. Delete files (best-effort — missing files are not an error).
    delete_declaration_files(storage, config, decl);

    // 3. Delete the declaration record.
    storage.delete_declaration(&decl.declaration_id)?;

    // 4. The ledger's record of this declaration is now obsolete.
    let mut ledger = storage.load_carryforward_ledger();
    let before = ledger.clone();
    prior_loss_use::forget(&mut ledger, decl);
    if ledger != before {
        storage
            .save_carryforward_ledger(&ledger)
            .with_context(|| storage.io_error_hint())?;
    }

    Ok(())
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
