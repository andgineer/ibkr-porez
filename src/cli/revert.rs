use anyhow::Result;

use super::{load_config_or_exit, make_storage, output};
use crate::RevertTarget;
use ibkr_porez::declaration_manager::DeclarationManager;

#[allow(clippy::needless_pass_by_value)]
pub fn run(declaration_id: &str, to: RevertTarget) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let manager = DeclarationManager::new(&storage);

    match to {
        RevertTarget::Draft => {
            manager.revert(&[declaration_id])?;
            output::success(&format!("Reverted {declaration_id} to draft"));
        }
        RevertTarget::Submitted => {
            manager.submit(&[declaration_id])?;
            output::success(&format!("Submitted: {declaration_id}"));
        }
    }

    Ok(())
}
