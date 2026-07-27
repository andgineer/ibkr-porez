use eframe::egui;

use super::app::App;

pub struct SubmitDialog {
    pub id: String,
    pub number: String,
}

impl SubmitDialog {
    #[must_use]
    pub fn new(id: String) -> Self {
        Self {
            id,
            number: String::new(),
        }
    }
}

pub fn show(ctx: &egui::Context, app: &mut App) {
    if app.submit_dialog.is_none() {
        return;
    }

    let mut submit = false;
    let mut dismiss = false;

    let dialog = app.submit_dialog.as_mut().unwrap();

    egui::Window::new("Submit Declaration")
        .collapsible(false)
        .resizable(false)
        .default_width(420.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let dim = ui.visuals().widgets.noninteractive.fg_stroke.color;

            ui.label(format!(
                "Mark declaration {} as submitted.",
                dialog.id.clone()
            ));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Declaration number at PURS:");
                ui.add(egui::TextEdit::singleline(&mut dialog.number).desired_width(180.0));
            });
            ui.colored_label(
                dim,
                "Optional, 1\u{2013}19 digits. An amendment of this declaration \
                 carries the number so the authority can tell which return it replaces.",
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Submit").clicked() {
                    submit = true;
                }
                if ui.button("Cancel").clicked() {
                    dismiss = true;
                }
            });
        });

    if submit {
        let dialog = app.submit_dialog.take().unwrap();
        let number = dialog.number.trim().to_string();
        app.row_submit(
            &dialog.id,
            if number.is_empty() {
                None
            } else {
                Some(number.as_str())
            },
        );
    } else if dismiss {
        app.submit_dialog = None;
    }
}
