pub mod app;
pub mod assessment_dialog;
pub mod carryforward_dialog;
pub mod config_dialog;
pub mod delete_dialog;
pub mod details_dialog;
pub mod icon;
pub mod import_dialog;
pub mod main_window;
pub mod styles;
pub mod sync_file_dialog;

/// Pin the application `mac-notification-sys` delivers desktop notifications as.
/// Without an explicit application it looks up one literally named `use_default`,
/// which makes `LaunchServices` pop a "Choose Application" dialog on the first
/// notification. Must match `CFBundleIdentifier` in `scripts/Info.plist.template`.
#[cfg(target_os = "macos")]
pub fn init_notifications() {
    let _ = notify_rust::set_application("engineer.sorokin.ibkr-porez");
}
