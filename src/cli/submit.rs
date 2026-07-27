use anyhow::{Result, bail};
use ibkr_porez::models::DeclarationStatus;

use super::{output, resolve_ids, run_bulk_resolved};

#[allow(clippy::needless_pass_by_value)]
pub fn run(declaration_id: Vec<String>, number: Option<String>) -> Result<()> {
    let ids = resolve_ids(declaration_id);
    if number.is_some() && ids.len() != 1 {
        bail!(
            "--number records the PURS number of one declaration; {} given",
            ids.len()
        );
    }

    run_bulk_resolved(&ids, |m, id| {
        m.submit_with_number(&[id], number.as_deref())?;
        let msg = match m.get_status(id) {
            Some(DeclarationStatus::Finalized) => {
                format!("Finalized: {id} (no tax to pay)")
            }
            Some(DeclarationStatus::Pending) => {
                format!("Submitted: {id} (pending tax authority assessment)")
            }
            _ => {
                format!("Submitted: {id}")
            }
        };
        output::success(&msg);
        Ok(())
    })
}
