use anyhow::Result;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::{load_config_or_exit, make_storage, output, validate_non_negative_decimal};
use ibkr_porez::declaration_manager::{AssessmentInput, DeclarationManager};

#[allow(clippy::too_many_arguments)]
pub fn run(
    declaration_id: &str,
    tax: Option<Decimal>,
    gain: Option<Decimal>,
    loss: Option<Decimal>,
    reference: Option<String>,
    assessment_date: Option<NaiveDate>,
    notes: Option<String>,
    paid: bool,
) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let manager = DeclarationManager::new(&storage);

    let tax = tax.map(validate_non_negative_decimal).transpose()?;
    let gain = gain.map(validate_non_negative_decimal).transpose()?;
    let loss = loss.map(validate_non_negative_decimal).transpose()?;

    let input = AssessmentInput {
        assessed_tax_rsd: tax,
        recognized_capital_gain_rsd: gain,
        recognized_capital_loss_rsd: loss,
        assessment_reference: reference,
        assessment_date,
        assessment_notes: notes,
        mark_paid: paid,
    };
    manager.record_assessment(declaration_id, &input)?;

    let decl = storage
        .get_declaration(declaration_id)
        .expect("declaration should exist after assessment");
    let msg = DeclarationManager::assessment_message(declaration_id, tax, decl.status, paid);
    output::success(&msg);

    if let Some(loss) = loss
        && loss > Decimal::ZERO
    {
        let vintage_id = format!("CF-{declaration_id}");
        if let Some(vintage) = storage.find_carryforward_vintage(&vintage_id) {
            output::info(&format!(
                "Carryforward vintage {} recorded: {:.2} RSD, expires after tax year {}.",
                vintage.id, vintage.recognized_loss_rsd, vintage.expiration_tax_year
            ));
        }
    }

    Ok(())
}
