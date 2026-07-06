use eframe::egui;

use crate::delete::{DeletePlan, execute_deletion};
use crate::models::DeclarationStatus;

use super::app::App;
use super::styles;

pub struct DeleteDialog {
    pub plan: DeletePlan,
    pub force: bool,
}

impl DeleteDialog {
    #[must_use]
    pub fn new(plan: DeletePlan) -> Self {
        Self { plan, force: false }
    }
}

pub fn show(ctx: &egui::Context, app: &mut App) {
    if app.delete_dialog.is_none() {
        return;
    }

    let mut execute = false;
    let mut dismiss = false;

    let dialog = app.delete_dialog.as_mut().unwrap();
    let decl = &dialog.plan.to_delete;
    let is_non_draft = decl.status != DeclarationStatus::Draft;

    egui::Window::new("Delete Declaration")
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let warn = ui.visuals().warn_fg_color;
            let dim = ui.visuals().widgets.noninteractive.fg_stroke.color;

            ui.label(format!(
                "Delete {} ({}), period {}, status {}, {} attachment(s).",
                decl.declaration_id,
                decl.display_type(),
                decl.display_period(),
                decl.status,
                decl.attached_files.len(),
            ));
            if dialog.plan.deletes_vintage {
                ui.colored_label(
                    warn,
                    "This also deletes its recognized-loss (assessment) data.",
                );
            }
            ui.add_space(4.0);
            ui.colored_label(
                dim,
                "Carryforward it consumed is returned to its vintages. \
                 Run Sync afterwards to rebuild the period if needed.",
            );

            if is_non_draft {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.checkbox(
                    &mut dialog.force,
                    format!("Force \u{2014} delete this {} declaration", decl.status),
                );
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let can_execute = !is_non_draft || dialog.force;
                ui.add_enabled_ui(can_execute, |ui| {
                    if ui.button("Delete").clicked() {
                        execute = true;
                    }
                });
                if ui.button("Cancel").clicked() {
                    dismiss = true;
                }
                if is_non_draft && !dialog.force {
                    ui.colored_label(warn, "Enable Force to delete a non-draft declaration.");
                }
            });
        });

    if execute {
        let dialog = app.delete_dialog.take().unwrap();
        let id = dialog.plan.to_delete.declaration_id.clone();
        match execute_deletion(&app.storage, &app.config, &dialog.plan) {
            Ok(()) => {
                let mut msg = format!("Deleted declaration {id}.");
                let kind = if dialog.plan.deletes_vintage {
                    msg.push_str(
                        " Assessment data was deleted \u{2014} re-run Set Tax if you recreate it.",
                    );
                    styles::MessageKind::Warning
                } else {
                    styles::MessageKind::Success
                };
                app.status_message = Some((msg, kind));
            }
            Err(e) => app.set_error(format!("{e:#}")),
        }
        app.refresh_declarations();
    } else if dismiss {
        app.delete_dialog = None;
    }
}
