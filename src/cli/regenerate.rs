use anyhow::{Result, bail};

use super::{init_calendar_with_sync, load_config_or_exit, make_nbs, make_storage, output};
use ibkr_porez::models::DeclarationStatus;
use ibkr_porez::regenerate::{execute_regeneration, plan_regeneration};

pub fn run(declaration_id: &str, yes: bool, force: bool) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);

    let plan = plan_regeneration(&storage, declaration_id)?;
    let decl = &plan.to_delete;

    output::bold("Regeneration plan:");
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

    let (start, end) = plan.period_to_generate;
    output::info(&format!("  Regenerate period {start} to {end}"));

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

    let cal = init_calendar_with_sync(&cfg);
    let nbs = make_nbs(&storage, &cal);

    let sp = output::spinner("Regenerating declaration...");
    let result = execute_regeneration(&storage, &nbs, &cfg, &cal, &plan, force);
    sp.finish_and_clear();
    let created = result?;

    if created.is_empty() {
        output::warning("No declaration recreated (period has nothing to declare).");
    } else {
        for d in &created {
            output::success(&format!(
                "Created declaration {} ({})",
                d.declaration_id,
                d.display_type()
            ));
        }
    }

    if plan.deletes_vintage {
        output::attention(
            "Assessment data was deleted; re-run `assess` on the regenerated declaration.",
        );
    }

    Ok(())
}
