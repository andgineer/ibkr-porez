use anyhow::{Result, bail};

use super::{load_config_or_exit, make_storage, output};
use ibkr_porez::delete::{execute_deletion, plan_deletion};
use ibkr_porez::models::DeclarationStatus;

pub fn run(declaration_id: &str, yes: bool, force: bool) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);

    let plan = plan_deletion(&storage, declaration_id)?;
    let decl = &plan.to_delete;

    output::bold("Deletion plan:");
    let mut delete_line = format!(
        "  Delete {} ({}), period {}, status {}, {} attachment(s)",
        decl.declaration_id,
        decl.display_type(),
        decl.display_period(),
        decl.status,
        decl.attached_files.len(),
    );
    if plan.deletes_vintage {
        delete_line.push_str(", has carryforward vintage");
    }
    output::warning(&delete_line);
    output::dim("  Run `sync` afterwards to rebuild the period if needed.");

    if decl.status != DeclarationStatus::Draft && !force {
        bail!(
            "declaration {declaration_id} is {}; pass --force to delete it",
            decl.status
        );
    }

    if !yes {
        output::dim("Dry run. Pass --yes to execute.");
        return Ok(());
    }

    execute_deletion(&storage, &cfg, &plan)?;
    output::success(&format!("Deleted declaration {declaration_id}."));

    if plan.deletes_vintage {
        output::attention(
            "Assessment data was deleted; re-run `assess` if you recreate this declaration.",
        );
    }

    Ok(())
}
