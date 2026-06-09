use eframe::egui;

use super::app::App;

pub struct SyncFileDialog {
    pub file_path: String,
}

impl Default for SyncFileDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncFileDialog {
    pub fn new() -> Self {
        Self {
            file_path: String::new(),
        }
    }
}

pub fn show(ctx: &egui::Context, app: &mut App) {
    if app.sync_file_dialog.is_none() {
        return;
    }

    let mut dismiss = false;
    let mut do_sync = false;

    let dialog = app.sync_file_dialog.as_mut().unwrap();

    egui::Window::new("Sync from File")
        .collapsible(false)
        .resizable(true)
        .default_width(500.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("File:");
                ui.text_edit_singleline(&mut dialog.file_path);
                if ui.small_button("Browse\u{2026}").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("XML (Flex)", &["xml"])
                        .pick_file()
                {
                    dialog.file_path = path.display().to_string();
                }
            });

            ui.add_space(12.0);
            ui.hyperlink_to(
                "How to download Flex Query XML from IBKR",
                "https://andgineer.github.io/ibkr-porez/en/ibkr.html\
                 #download-flex-query-xml-for-sync---file",
            );

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!dialog.file_path.is_empty(), |ui| {
                    if ui.button("Sync").clicked() {
                        do_sync = true;
                    }
                });
                if ui.button("Close").clicked() {
                    dismiss = true;
                }
            });
        });

    if do_sync {
        let path =
            std::path::PathBuf::from(app.sync_file_dialog.as_ref().unwrap().file_path.clone());
        app.sync_file_dialog = None;
        app.start_sync_from_file(path);
    } else if dismiss {
        app.sync_file_dialog = None;
    }
}
