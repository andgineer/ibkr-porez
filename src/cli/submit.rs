use anyhow::Result;

use super::{load_config_or_exit, make_storage, output};
use ibkr_porez::declaration_manager::DeclarationManager;
use ibkr_porez::models::DeclarationStatus;

pub fn run(declaration_id: &str) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let manager = DeclarationManager::new(&storage);

    manager.submit(&[declaration_id])?;

    let decl = storage
        .get_declaration(declaration_id)
        .expect("declaration should exist after submit");

    let msg = match decl.status {
        DeclarationStatus::Finalized => {
            format!("Finalized: {declaration_id} (no tax to pay)")
        }
        DeclarationStatus::Pending => {
            format!("Submitted: {declaration_id} (pending tax authority assessment)")
        }
        _ => {
            format!("Submitted: {declaration_id}")
        }
    };
    output::success(&msg);
    Ok(())
}
