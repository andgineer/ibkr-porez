use chrono::NaiveDate;
use eframe::egui;
use rust_decimal::Decimal;

use crate::declaration_manager::{AssessmentInput, DeclarationManager};
use crate::models::DeclarationType;

use super::app::App;

pub struct AssessmentDialog {
    pub declaration_id: String,
    pub decl_type: DeclarationType,
    pub tax_input: String,
    pub mark_paid: bool,
    pub recognized_gain_input: String,
    pub recognized_loss_input: String,
    pub reference_input: String,
    pub assessment_date_input: String,
    pub notes_input: String,
}

impl AssessmentDialog {
    pub fn new(id: String, decl_type: DeclarationType) -> Self {
        Self {
            declaration_id: id,
            decl_type,
            tax_input: String::new(),
            mark_paid: false,
            recognized_gain_input: String::new(),
            recognized_loss_input: String::new(),
            reference_input: String::new(),
            assessment_date_input: String::new(),
            notes_input: String::new(),
        }
    }
}

fn parse_non_negative_decimal(input: &str, field: &str) -> Result<Option<Decimal>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match trimmed.parse::<Decimal>() {
        Ok(v) if v < Decimal::ZERO => Err(format!("{field} must be non-negative")),
        Ok(v) => Ok(Some(v.round_dp(2))),
        Err(_) => Err(format!("{field}: enter a valid number")),
    }
}

fn parse_date(input: &str) -> Result<Option<NaiveDate>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| "Assessment date: use YYYY-MM-DD format".to_string())
}

fn non_empty(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn show(ctx: &egui::Context, app: &mut App) {
    if app.assessment_dialog.is_none() {
        return;
    }

    let mut dismiss = false;
    let mut apply = false;

    let dialog = app.assessment_dialog.as_mut().unwrap();
    let is_ppdg3r = dialog.decl_type == DeclarationType::Ppdg3r;

    egui::Window::new("Tax Assessment")
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Tax due (RSD):");
                ui.text_edit_singleline(&mut dialog.tax_input);
            });
            ui.checkbox(&mut dialog.mark_paid, "Already paid");

            if is_ppdg3r {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.colored_label(
                    ui.visuals().widgets.noninteractive.fg_stroke.color,
                    "Tax authority assessment (optional)",
                );
                ui.horizontal(|ui| {
                    ui.label("Recognized gain (RSD):");
                    ui.text_edit_singleline(&mut dialog.recognized_gain_input);
                });
                ui.horizontal(|ui| {
                    ui.label("Recognized loss (RSD):");
                    ui.text_edit_singleline(&mut dialog.recognized_loss_input);
                });
                ui.horizontal(|ui| {
                    ui.label("Reference:");
                    ui.text_edit_singleline(&mut dialog.reference_input);
                });
                ui.horizontal(|ui| {
                    ui.label("Assessment date (YYYY-MM-DD):");
                    ui.text_edit_singleline(&mut dialog.assessment_date_input);
                });
                ui.horizontal(|ui| {
                    ui.label("Notes:");
                    ui.text_edit_singleline(&mut dialog.notes_input);
                });
                ui.add_space(4.0);
                ui.colored_label(
                    ui.visuals().widgets.noninteractive.fg_stroke.color,
                    "A recognized loss is recorded as a carryforward vintage \
                     and will be applied against future capital gains.",
                );
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    dismiss = true;
                }
            });
        });

    if apply {
        let dialog = app.assessment_dialog.take().unwrap();
        match build_assessment_input(&dialog) {
            Ok(input) => {
                let manager = DeclarationManager::new(&app.storage);
                if let Err(e) = manager.record_assessment(&dialog.declaration_id, &input) {
                    app.error_dialog = Some(e.to_string());
                }
                app.refresh_declarations();
            }
            Err(e) => {
                app.error_dialog = Some(e);
            }
        }
    } else if dismiss {
        app.assessment_dialog = None;
    }
}

fn build_assessment_input(dialog: &AssessmentDialog) -> Result<AssessmentInput, String> {
    let assessed_tax_rsd = parse_non_negative_decimal(&dialog.tax_input, "Tax due")?;
    let recognized_capital_gain_rsd =
        parse_non_negative_decimal(&dialog.recognized_gain_input, "Recognized gain")?;
    let recognized_capital_loss_rsd =
        parse_non_negative_decimal(&dialog.recognized_loss_input, "Recognized loss")?;
    let assessment_date = parse_date(&dialog.assessment_date_input)?;

    Ok(AssessmentInput {
        assessed_tax_rsd,
        recognized_capital_gain_rsd,
        recognized_capital_loss_rsd,
        assessment_reference: non_empty(&dialog.reference_input),
        assessment_date,
        assessment_notes: non_empty(&dialog.notes_input),
        mark_paid: dialog.mark_paid,
    })
}
