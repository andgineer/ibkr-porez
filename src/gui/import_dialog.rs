use std::sync::mpsc;

use eframe::egui;

use crate::import::FileType;

use super::app::{App, BackgroundResult};

pub struct ImportDialog {
    pub file_path: String,
    pub file_type: FileType,
    pub busy: bool,
}

impl Default for ImportDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportDialog {
    pub fn new() -> Self {
        Self {
            file_path: String::new(),
            file_type: FileType::Auto,
            busy: false,
        }
    }
}

pub fn show(ctx: &egui::Context, app: &mut App) {
    if app.import_dialog.is_none() {
        return;
    }

    let mut dismiss = false;
    let mut do_import = false;

    let dialog = app.import_dialog.as_mut().unwrap();

    egui::Window::new("Import Transactions")
        .collapsible(false)
        .resizable(true)
        .default_width(450.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("Source File");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("File:");
                ui.text_edit_singleline(&mut dialog.file_path);
                if ui.small_button("Browse...").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("All supported", &["csv", "xml"])
                        .add_filter("CSV", &["csv"])
                        .add_filter("XML (Flex)", &["xml"])
                        .pick_file()
                {
                    dialog.file_path = path.display().to_string();
                }
            });

            ui.add_space(12.0);
            ui.heading("Format");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.radio_value(&mut dialog.file_type, FileType::Auto, "Auto");
                ui.radio_value(&mut dialog.file_type, FileType::Csv, "CSV");
                ui.radio_value(&mut dialog.file_type, FileType::Flex, "Flex XML");
            });

            ui.add_space(12.0);
            ui.colored_label(
                ui.visuals().widgets.noninteractive.fg_stroke.color,
                "Import is needed for transactions older than one year to calculate \
                 stock sale income correctly. Data within the last year is fetched by Sync.",
            );
            ui.add_space(4.0);
            ui.hyperlink_to(
                "Import documentation",
                "https://andgineer.github.io/ibkr-porez/en/usage.html",
            );

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                let can_import = !dialog.file_path.is_empty() && !dialog.busy;
                ui.add_enabled_ui(can_import, |ui| {
                    if ui.button("Import").clicked() {
                        do_import = true;
                    }
                });
                if ui.button("Close").clicked() {
                    dismiss = true;
                }
                if dialog.busy {
                    ui.spinner();
                }
            });
        });

    if do_import {
        start_import(app);
    } else if dismiss {
        app.import_dialog = None;
    }
}

fn start_import(app: &mut App) {
    let dialog = app.import_dialog.as_mut().unwrap();
    dialog.busy = true;

    let path = dialog.file_path.clone();
    let file_type = dialog.file_type;
    let (tx, rx) = mpsc::channel();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;
    app.progress_text = Some("Importing\u{2026}".into());

    std::thread::spawn(move || {
        let storage = crate::storage::Storage::new();
        let holidays = crate::holidays::HolidayCalendar::load_embedded();
        let nbs = crate::nbs::NBSClient::new(&storage, &holidays);
        let result = crate::import::import_from_file(
            &storage,
            &nbs,
            &std::path::PathBuf::from(path),
            file_type,
        )
        .map_err(|e| format!("{e:#}"));
        let _ = tx.send(BackgroundResult::ImportDone(result));
    });
}
