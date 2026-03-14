use anyhow::Result;
use rust_decimal::Decimal;

use super::{load_config_or_exit, make_storage, output, validate_non_negative_decimal};
use ibkr_porez::declaration_manager::DeclarationManager;

pub fn run(declaration_id: &str, tax: Option<Decimal>) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let manager = DeclarationManager::new(&storage);

    if let Some(amount) = tax {
        let amount = validate_non_negative_decimal(amount)?;
        manager.set_assessed_tax(declaration_id, amount, true)?;
        output::success(&format!(
            "Paid: {declaration_id} ({amount:.2} RSD recorded)"
        ));
    } else {
        manager.pay(&[declaration_id])?;
        output::success(&format!("Paid: {declaration_id}"));
    }

    Ok(())
}
