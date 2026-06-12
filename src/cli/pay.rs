use anyhow::{Result, bail};
use rust_decimal::Decimal;

use super::{output, resolve_ids, run_bulk, run_bulk_resolved, validate_non_negative_decimal};
use ibkr_porez::declaration_manager::AssessmentInput;

pub fn run(declaration_id: Vec<String>, tax: Option<Decimal>) -> Result<()> {
    if let Some(raw_amount) = tax {
        let amount = validate_non_negative_decimal(raw_amount)?;
        let ids = resolve_ids(declaration_id);
        if ids.len() > 1 {
            bail!("--tax can only be used with a single declaration ID");
        }
        return run_bulk_resolved(&ids, |m, id| {
            let input = AssessmentInput {
                assessed_tax_rsd: Some(amount),
                mark_paid: true,
                ..Default::default()
            };
            m.record_assessment(id, &input)?;
            output::success(&format!("Paid: {id} ({amount:.2} RSD recorded)"));
            Ok(())
        });
    }

    run_bulk(declaration_id, |m, id| {
        m.pay(&[id])?;
        output::success(&format!("Paid: {id}"));
        Ok(())
    })
}
