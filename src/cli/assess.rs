use anyhow::Result;
use rust_decimal::Decimal;

use super::{load_config_or_exit, make_storage, output, validate_non_negative_decimal};
use ibkr_porez::declaration_manager::DeclarationManager;

pub fn run(declaration_id: &str, tax_due: Decimal, paid: bool) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let manager = DeclarationManager::new(&storage);

    let tax_due = validate_non_negative_decimal(tax_due)?;
    manager.set_assessed_tax(declaration_id, tax_due, paid)?;

    let decl = storage
        .get_declaration(declaration_id)
        .expect("declaration should exist after assessment");
    let msg = DeclarationManager::assessment_message(declaration_id, tax_due, decl.status, paid);
    output::success(&msg);
    Ok(())
}
