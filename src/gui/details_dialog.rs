use crate::storage::Storage;
use eframe::egui;

use super::app::App;

pub struct DetailsDialog {
    pub declaration_id: String,
    pub text: String,
}

impl DetailsDialog {
    pub fn new(id: &str, storage: &Storage) -> Self {
        let text = if let Some(decl) = storage.get_declaration(id) {
            format_declaration(&decl)
        } else {
            format!("Declaration {id} not found.")
        };
        Self {
            declaration_id: id.to_string(),
            text,
        }
    }
}

pub fn show(ctx: &egui::Context, app: &mut App) {
    let Some(ref _dialog) = app.details_dialog else {
        return;
    };

    let mut dismiss = false;

    egui::Window::new("Declaration Details")
        .collapsible(false)
        .resizable(true)
        .default_width(600.0)
        .default_height(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let dialog = app.details_dialog.as_ref().unwrap();
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut dialog.text.as_str())
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
            });
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                dismiss = true;
            }
        });

    if dismiss {
        app.details_dialog = None;
    }
}

pub fn format_declaration(decl: &crate::models::Declaration) -> String {
    let mut lines = Vec::new();
    lines.push(format!("ID:       {}", decl.declaration_id));
    lines.push(format!("Type:     {}", decl.display_type()));
    lines.push(format!("Period:   {}", decl.display_period()));
    lines.push(format!("Status:   {}", decl.status));
    lines.push(format!("Tax:      {}", decl.display_tax()));
    lines.push(format!(
        "Created:  {}",
        decl.created_at.format("%Y-%m-%d %H:%M:%S")
    ));
    if let Some(ref dt) = decl.submitted_at {
        lines.push(format!("Submitted: {}", dt.format("%Y-%m-%d %H:%M:%S")));
    }
    if let Some(ref dt) = decl.paid_at {
        lines.push(format!("Paid:     {}", dt.format("%Y-%m-%d %H:%M:%S")));
    }

    if !decl.metadata.is_empty() {
        lines.push(String::new());
        lines.push("--- Metadata ---".to_string());
        for (k, v) in &decl.metadata {
            let val = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            lines.push(format!("{k}: {val}"));
        }
    }

    if !decl.attached_files.is_empty() {
        lines.push(String::new());
        lines.push("--- Attached Files ---".to_string());
        for (name, path) in &decl.attached_files {
            lines.push(format!("{name}: {path}"));
        }
    }

    lines.join("\n")
}
