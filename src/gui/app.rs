use std::path::PathBuf;
use std::sync::mpsc;

use chrono::Datelike;

use crate::config as app_config;
use crate::declaration_manager::DeclarationManager;
use crate::holidays::HolidayCalendar;
use crate::list::{ListOptions, list_declarations};
use crate::models::{Declaration, DeclarationStatus, UserConfig};
use crate::storage::Storage;
use eframe::egui;

use super::assessment_dialog::AssessmentDialog;
use super::config_dialog::ConfigDialog;
use super::details_dialog::DetailsDialog;
use super::import_dialog::ImportDialog;
use super::main_window;
use super::styles;
use super::sync_file_dialog::SyncFileDialog;

fn notify_new_declarations(count: usize) {
    let body = if count == 1 {
        "1 new declaration was created".to_string()
    } else {
        format!("{count} new declarations were created")
    };
    let _ = notify_rust::Notification::new()
        .summary("ibkr-porez")
        .body(&body)
        .show();
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterScope {
    Active,
    All,
    PendingPayment,
}

impl FilterScope {
    pub const ALL: &[Self] = &[Self::Active, Self::All, Self::PendingPayment];

    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::All => "All",
            Self::PendingPayment => "Pending payment",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BulkAction {
    Submit,
    Pay,
    Revert,
}

impl BulkAction {
    pub const ALL: &[Self] = &[Self::Submit, Self::Pay, Self::Revert];

    pub fn label(self) -> &'static str {
        match self {
            Self::Submit => "Submit",
            Self::Pay => "Pay",
            Self::Revert => "Revert to Draft",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortColumn {
    Id,
    Type,
    Period,
    Tax,
    Status,
    Created,
}

impl SortColumn {
    pub const ALL: &[Self] = &[
        Self::Id,
        Self::Type,
        Self::Period,
        Self::Tax,
        Self::Status,
        Self::Created,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Id => "ID",
            Self::Type => "Type",
            Self::Period => "Period",
            Self::Tax => "Tax",
            Self::Status => "Status",
            Self::Created => "Created",
        }
    }
}

pub enum BackgroundResult {
    SyncDone(Result<crate::sync::SyncResult, String>),
    ImportDone(Result<crate::import::ImportResult, String>),
    ExportDone {
        id: String,
        result: Result<String, String>,
    },
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    pub config: UserConfig,
    pub config_file: PathBuf,
    pub storage: Storage,
    pub declarations: Vec<Declaration>,
    pub selected: std::collections::HashSet<String>,

    pub filter: FilterScope,
    pub bulk_action: BulkAction,

    pub sort_column: SortColumn,
    pub sort_ascending: bool,

    pub status_message: Option<(String, styles::MessageKind)>,
    pub warning_banner: Option<String>,

    pub bg_receiver: Option<mpsc::Receiver<BackgroundResult>>,
    pub bg_busy: bool,
    pub last_sync_success: Option<chrono::NaiveDateTime>,
    pub last_sync_issue: Option<(chrono::NaiveDateTime, String)>,
    last_sync_fatal: bool,
    pub pending_new_declarations: u32,
    ctx: egui::Context,
    auto_sync_tx: mpsc::Sender<()>,
    auto_sync_rx: mpsc::Receiver<()>,
    scheduler_started: bool,
    pub export_channel: Option<(
        mpsc::Sender<BackgroundResult>,
        mpsc::Receiver<BackgroundResult>,
    )>,
    pub exporting_ids: std::collections::HashSet<String>,

    pub progress_text: Option<String>,
    pub confirm_force_sync: bool,

    pub config_dialog: Option<ConfigDialog>,
    pub import_dialog: Option<ImportDialog>,
    pub sync_file_dialog: Option<SyncFileDialog>,
    pub details_dialog: Option<DetailsDialog>,
    pub assessment_dialog: Option<AssessmentDialog>,
    pub error_dialog: Option<String>,
    pub show_import_hint: bool,
    pub confirm_discard_config: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let config_file = app_config::config_file_path();
        let config = app_config::load_config_from(&config_file);
        let storage = Storage::with_config(&config);
        let declarations = load_filtered(&storage, FilterScope::Active);

        let warning_banner = check_holiday_warning(&config);
        let show_import_hint = storage.get_last_transaction_date().is_none();

        let last_sync_success = storage.get_last_sync_success();
        let last_sync_issue = storage.get_last_sync_issue();
        let pending_new_declarations = storage.get_pending_new_declarations();

        let (auto_sync_tx, auto_sync_rx) = mpsc::channel::<()>();
        let _ = auto_sync_tx.send(());

        Self {
            config,
            config_file,
            storage,
            declarations,
            selected: std::collections::HashSet::new(),
            filter: FilterScope::Active,
            bulk_action: BulkAction::Submit,
            sort_column: SortColumn::Created,
            sort_ascending: false,
            status_message: None,
            warning_banner,
            bg_receiver: None,
            bg_busy: false,
            last_sync_success,
            last_sync_issue,
            last_sync_fatal: false,
            pending_new_declarations,
            ctx: egui::Context::default(),
            auto_sync_tx,
            auto_sync_rx,
            scheduler_started: false,
            export_channel: None,
            exporting_ids: std::collections::HashSet::new(),
            progress_text: None,
            confirm_force_sync: false,
            config_dialog: None,
            import_dialog: None,
            sync_file_dialog: None,
            details_dialog: None,
            assessment_dialog: None,
            error_dialog: None,
            show_import_hint,
            confirm_discard_config: false,
        }
    }

    pub fn reload_config(&mut self) {
        self.config = app_config::load_config_from(&self.config_file);
    }

    pub fn reload_storage(&mut self) {
        self.storage = Storage::with_config(&self.config);
    }

    pub fn refresh_declarations(&mut self) {
        self.reload_config();
        self.reload_storage();
        self.declarations = load_filtered(&self.storage, self.filter);
        self.sort_declarations();
        self.selected
            .retain(|id| self.declarations.iter().any(|d| &d.declaration_id == id));
        self.show_import_hint = self.storage.get_last_transaction_date().is_none();
    }

    pub fn sort_declarations(&mut self) {
        let col = self.sort_column;
        let asc = self.sort_ascending;
        self.declarations.sort_by(|a, b| {
            let ord = match col {
                SortColumn::Id => a.declaration_id.cmp(&b.declaration_id),
                SortColumn::Type => a.display_type().cmp(b.display_type()),
                SortColumn::Period => a.period_start.cmp(&b.period_start),
                SortColumn::Tax => a.display_tax().cmp(&b.display_tax()),
                SortColumn::Status => a.status.to_string().cmp(&b.status.to_string()),
                SortColumn::Created => a.created_at.cmp(&b.created_at),
            };
            if asc { ord } else { ord.reverse() }
        });
    }

    pub fn set_sort(&mut self, col: SortColumn) {
        if self.sort_column == col {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = col;
            self.sort_ascending = true;
        }
        self.sort_declarations();
    }

    pub fn set_filter(&mut self, scope: FilterScope) {
        self.filter = scope;
        self.refresh_declarations();
    }

    pub fn select_all(&mut self) {
        for d in &self.declarations {
            self.selected.insert(d.declaration_id.clone());
        }
    }

    pub fn unselect_all(&mut self) {
        self.selected.clear();
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        tracing::error!("{msg}");
        self.error_dialog = Some(msg);
    }

    pub fn apply_bulk_action(&mut self) {
        let manager = DeclarationManager::new(&self.storage);
        let ids: Vec<String> = self.selected.iter().cloned().collect();
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();

        let bulk_action = self.bulk_action;
        let result = manager.apply_each(&id_refs, |m, id| match bulk_action {
            BulkAction::Submit => m.submit(&[id]),
            BulkAction::Pay => m.pay(&[id]),
            BulkAction::Revert => m.revert(&[id]),
        });

        if result.ok_count > 0 {
            self.status_message = Some((
                format!(
                    "{} applied to {} declaration(s)",
                    self.bulk_action.label(),
                    result.ok_count,
                ),
                styles::MessageKind::Success,
            ));
        }
        if result.has_errors() {
            self.set_error(result.error_summary());
        }
        if !result.has_errors() {
            self.selected.clear();
        }
        self.refresh_declarations();
    }

    pub fn row_submit(&mut self, id: &str) {
        let manager = DeclarationManager::new(&self.storage);
        if let Err(e) = manager.submit(&[id]) {
            self.set_error(format!("{e:#}"));
        }
        self.refresh_declarations();
    }

    pub fn row_pay(&mut self, id: &str) {
        let manager = DeclarationManager::new(&self.storage);
        if let Err(e) = manager.pay(&[id]) {
            self.set_error(format!("{e:#}"));
        }
        self.refresh_declarations();
    }

    pub fn row_revert(&mut self, id: &str) {
        let manager = DeclarationManager::new(&self.storage);
        if let Err(e) = manager.revert(&[id]) {
            self.set_error(format!("{e:#}"));
        }
        self.refresh_declarations();
    }

    fn export_sender(&mut self) -> mpsc::Sender<BackgroundResult> {
        if let Some((ref tx, _)) = self.export_channel {
            return tx.clone();
        }
        let (tx, rx) = mpsc::channel();
        self.export_channel = Some((tx.clone(), rx));
        tx
    }

    pub fn row_export(&mut self, id: String) {
        if self.exporting_ids.contains(&id) {
            return;
        }
        self.exporting_ids.insert(id.clone());
        let config = self.config.clone();
        let tx = self.export_sender();
        let ctx = self.ctx.clone();

        std::thread::spawn(move || {
            let output_dir = app_config::get_effective_output_dir_path(&config);
            let storage = Storage::with_config(&config);
            let manager = DeclarationManager::new(&storage);
            let result = match manager.export(&id, &output_dir) {
                Ok(r) => {
                    let path = r
                        .xml_path
                        .unwrap_or_else(|| output_dir.display().to_string());
                    Ok(path)
                }
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(BackgroundResult::ExportDone { id, result });
            ctx.request_repaint();
        });
    }

    pub fn start_sync(&mut self, force: bool) {
        if self.bg_busy {
            return;
        }
        self.reload_config();

        let issues = app_config::validate_config(&self.config);
        if !issues.is_empty() {
            self.set_error(app_config::format_config_issues(&issues));
            return;
        }

        if self
            .warning_banner
            .as_deref()
            .is_some_and(|b| b.starts_with("Not configured"))
        {
            self.warning_banner = check_holiday_warning(&self.config);
        }

        self.bg_busy = true;
        self.status_message = None;
        self.progress_text = Some("Syncing…".into());
        let config = self.config.clone();
        let (tx, rx) = mpsc::channel();
        self.bg_receiver = Some(rx);
        let ctx = self.ctx.clone();

        std::thread::spawn(move || {
            let storage = Storage::with_config(&config);
            let mut holidays = crate::holidays::HolidayCalendar::load_embedded();
            let data_dir = app_config::get_effective_data_dir_path(&config);
            holidays.merge_file(&data_dir);
            if force {
                holidays.set_fallback(true);
            }
            let nbs = crate::nbs::NBSClient::new(&storage, &holidays);
            let ibkr = crate::ibkr_flex::IBKRClient::new(&config.ibkr_token, &config.ibkr_query_id);
            let opts = crate::sync::SyncOptions {
                force,
                ..Default::default()
            };
            let result = crate::sync::run_sync(&storage, &nbs, &config, &holidays, &opts, &ibkr)
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(BackgroundResult::SyncDone(result));
            ctx.request_repaint();
        });
    }

    pub fn start_sync_from_file(&mut self, path: PathBuf) {
        self.bg_busy = true;
        self.status_message = None;
        self.progress_text = Some("Syncing from file\u{2026}".into());
        let config = self.config.clone();
        let (tx, rx) = mpsc::channel();
        self.bg_receiver = Some(rx);
        let ctx = self.ctx.clone();

        std::thread::spawn(move || {
            let storage = Storage::with_config(&config);
            let mut holidays = crate::holidays::HolidayCalendar::load_embedded();
            let data_dir = app_config::get_effective_data_dir_path(&config);
            holidays.merge_file(&data_dir);
            let nbs = crate::nbs::NBSClient::new(&storage, &holidays);
            let opts = crate::sync::SyncOptions {
                force: false,
                ..Default::default()
            };
            let result =
                crate::sync::run_sync_from_file(&path, &storage, &nbs, &config, &holidays, &opts)
                    .map_err(|e| format!("{e:#}"));
            let _ = tx.send(BackgroundResult::SyncDone(result));
            ctx.request_repaint();
        });
    }

    /// True once a sync has succeeded today — the daily auto-cycle is done
    /// for today and resumes only after local midnight.
    pub fn synced_today(&self) -> bool {
        let today = chrono::Local::now().date_naive();
        self.last_sync_success.is_some_and(|dt| dt.date() == today)
    }

    pub fn poll_background(&mut self) {
        while let Ok(()) = self.auto_sync_rx.try_recv() {
            let now = chrono::Local::now().naive_local();
            if self.synced_today() {
                continue;
            }
            if app_config::validate_config(&self.config).is_empty() {
                let already_failed_fatally_today = self.last_sync_fatal
                    && self
                        .last_sync_issue
                        .as_ref()
                        .is_some_and(|(dt, _)| dt.date() == now.date());
                if !self.bg_busy && !already_failed_fatally_today {
                    self.start_sync(false);
                }
            } else {
                self.warning_banner =
                    Some("Not configured \u{2014} open Config to set up IBKR token".to_string());
            }
        }

        if let Some(ref rx) = self.bg_receiver {
            match rx.try_recv() {
                Ok(BackgroundResult::SyncDone(result)) => {
                    self.bg_busy = false;
                    self.bg_receiver = None;
                    self.progress_text = None;
                    self.handle_sync_done(result);
                    self.refresh_declarations();
                }
                Ok(BackgroundResult::ImportDone(result)) => {
                    self.bg_busy = false;
                    self.bg_receiver = None;
                    self.progress_text = None;
                    match result {
                        Ok(r) => {
                            self.status_message = Some((
                                format!(
                                    "Import complete: {} inserted, {} updated ({} total)",
                                    r.inserted, r.updated, r.transaction_count
                                ),
                                styles::MessageKind::Success,
                            ));
                        }
                        Err(e) => {
                            self.set_error(e);
                            self.status_message = None;
                        }
                    }
                    self.import_dialog = None;
                    self.refresh_declarations();
                }
                Ok(BackgroundResult::ExportDone { .. }) | Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.bg_busy = false;
                    self.bg_receiver = None;
                }
            }
        }

        if let Some((_, ref rx)) = self.export_channel {
            match rx.try_recv() {
                Ok(BackgroundResult::ExportDone { id, result }) => {
                    self.exporting_ids.remove(&id);
                    if self.exporting_ids.is_empty() {
                        self.export_channel = None;
                    }
                    match result {
                        Ok(path) => {
                            self.status_message =
                                Some((format!("Exported to {path}"), styles::MessageKind::Success));
                        }
                        Err(e) => {
                            self.set_error(e);
                        }
                    }
                }
                Ok(_) | Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.exporting_ids.clear();
                    self.export_channel = None;
                }
            }
        }
    }

    fn handle_sync_done(&mut self, result: Result<crate::sync::SyncResult, String>) {
        let now = chrono::Local::now().naive_local();
        match result {
            Ok(r) => {
                let _ = self.storage.set_last_sync_success(now);
                self.last_sync_success = Some(now);
                self.last_sync_fatal = false;
                if let Some(msg) = &r.income_error {
                    let _ = self.storage.set_last_sync_issue(now, msg);
                    self.last_sync_issue = Some((now, msg.clone()));
                } else {
                    let _ = self.storage.clear_last_sync_issue();
                    self.last_sync_issue = None;
                }
                self.warning_banner = check_holiday_warning(&self.config);

                let count = r.created_declarations.len();
                if count > 0 {
                    let count_u32 = u32::try_from(count).unwrap_or(u32::MAX);
                    let _ = self.storage.add_pending_new_declarations(count_u32);
                    self.pending_new_declarations = self.storage.get_pending_new_declarations();
                    notify_new_declarations(count);
                }
            }
            Err(e) => {
                let (display_message, should_retry) = classify_sync_error(&e, self.synced_today());
                let _ = self.storage.set_last_sync_issue(now, &display_message);
                self.last_sync_issue = Some((now, display_message));
                self.last_sync_fatal = !should_retry;
            }
        }
    }
}

/// Every error is retried automatically at least once a day, so the message
/// never claims "won't retry" — only whether it's hourly (now) or tomorrow.
fn classify_sync_error(e: &str, synced_today: bool) -> (String, bool) {
    let (reason, should_retry) = if e.contains("IBKR API Error 1001:")
        || e.contains("IBKR API Error 1018:")
        || e.contains("IBKR API Error 1019:")
    {
        ("Flex Query temporarily unavailable".to_string(), true)
    } else if e.contains("IBKR SendRequest failed")
        || e.contains("IBKR GetStatement request failed")
        || e.contains("IBKR SendRequest HTTP error")
        || e.contains("IBKR GetStatement HTTP error")
        || e.contains("GetStatement: not ready after")
    {
        ("Connection to IBKR failed".to_string(), true)
    } else {
        (e.to_string(), false)
    };

    let message = if should_retry && !synced_today {
        format!("{reason} \u{2014} retrying automatically.")
    } else {
        format!(
            "{reason} \u{2014} next automatic sync: tomorrow; click \"Sync now\" to try again sooner."
        )
    };
    (message, should_retry)
}

impl App {
    pub fn new_for_test(
        config: crate::models::UserConfig,
        config_file: std::path::PathBuf,
        storage: crate::storage::Storage,
        declarations: Vec<crate::models::Declaration>,
    ) -> Self {
        let (auto_sync_tx, auto_sync_rx) = mpsc::channel::<()>();
        Self {
            config,
            config_file,
            storage,
            declarations,
            selected: std::collections::HashSet::new(),
            filter: FilterScope::Active,
            bulk_action: BulkAction::Submit,
            sort_column: SortColumn::Created,
            sort_ascending: false,
            status_message: None,
            warning_banner: None,
            bg_receiver: None,
            bg_busy: false,
            last_sync_success: None,
            last_sync_issue: None,
            last_sync_fatal: false,
            pending_new_declarations: 0,
            export_channel: None,
            exporting_ids: std::collections::HashSet::new(),
            progress_text: None,
            confirm_force_sync: false,
            config_dialog: None,
            import_dialog: None,
            sync_file_dialog: None,
            details_dialog: None,
            assessment_dialog: None,
            error_dialog: None,
            show_import_hint: false,
            confirm_discard_config: false,
            ctx: egui::Context::default(),
            auto_sync_tx,
            auto_sync_rx,
            scheduler_started: true,
        }
    }

    pub fn trigger_sync_check(&self) {
        let _ = self.auto_sync_tx.send(());
        self.ctx.request_repaint();
    }

    pub fn dismiss_pending_new_declarations(&mut self) {
        let _ = self.storage.clear_pending_new_declarations();
        self.pending_new_declarations = 0;
    }
}

impl App {
    fn show_modal_dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_force_sync {
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new("Confirm Force Sync")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        "Force sync will re-fetch all data from IBKR and NBS.\n\
                         This may overwrite locally modified declarations.\n\n\
                         Are you sure?",
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Yes, force sync").clicked() {
                            confirm = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            if confirm {
                self.confirm_force_sync = false;
                self.start_sync(true);
            } else if cancel {
                self.confirm_force_sync = false;
            }
        }

        if self.confirm_discard_config {
            let mut discard = false;
            let mut stay = false;
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        "You have unsaved changes in the configuration.\n\
                         Discard them?",
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Discard").clicked() {
                            discard = true;
                        }
                        if ui.button("Keep editing").clicked() {
                            stay = true;
                        }
                    });
                });
            if discard {
                self.confirm_discard_config = false;
                self.config_dialog = None;
            } else if stay {
                self.confirm_discard_config = false;
            }
        }

        if let Some(msg) = self.error_dialog.clone() {
            let mut dismiss = false;
            egui::Window::new("Error")
                .collapsible(false)
                .resizable(false)
                .default_width(350.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    ui.label(&msg);
                    ui.add_space(12.0);
                    if ui.button("OK").clicked() {
                        dismiss = true;
                    }
                });
            if dismiss {
                self.error_dialog = None;
            }
        }
    }

    pub fn render(&mut self, ctx: &egui::Context) {
        self.ctx = ctx.clone();

        if !self.scheduler_started {
            self.scheduler_started = true;
            let tx = self.auto_sync_tx.clone();
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_hours(1));
                    let _ = tx.send(());
                    ctx.request_repaint();
                }
            });
        }

        self.poll_background();

        let modal_open = self.config_dialog.is_some()
            || self.import_dialog.is_some()
            || self.sync_file_dialog.is_some()
            || self.details_dialog.is_some()
            || self.assessment_dialog.is_some()
            || self.error_dialog.is_some()
            || self.confirm_force_sync
            || self.confirm_discard_config;

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(
                        ui.visuals().widgets.noninteractive.fg_stroke.color,
                        "Double-click a row to open details, or a column header to sort",
                    );
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(!modal_open, |ui| {
                main_window::show(ui, self);
            });
        });

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.error_dialog.is_some() {
                self.error_dialog = None;
            } else if self.confirm_discard_config {
                self.confirm_discard_config = false;
            } else if self.confirm_force_sync {
                self.confirm_force_sync = false;
            } else if self.assessment_dialog.is_some() {
                self.assessment_dialog = None;
            } else if self.details_dialog.is_some() {
                self.details_dialog = None;
            } else if self.sync_file_dialog.is_some() {
                self.sync_file_dialog = None;
            } else if self.import_dialog.is_some() && !self.bg_busy {
                self.import_dialog = None;
            } else if let Some(ref dialog) = self.config_dialog {
                if dialog.has_changes() {
                    self.confirm_discard_config = true;
                } else {
                    self.config_dialog = None;
                }
            }
        }

        super::config_dialog::show(ctx, self);
        super::import_dialog::show(ctx, self);
        super::sync_file_dialog::show(ctx, self);
        super::details_dialog::show(ctx, self);
        super::assessment_dialog::show(ctx, self);

        self.show_modal_dialogs(ctx);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render(ctx);
    }
}

pub fn load_filtered(storage: &Storage, scope: FilterScope) -> Vec<Declaration> {
    match scope {
        FilterScope::All => list_declarations(
            storage,
            &ListOptions {
                show_all: true,
                status: None,
            },
        ),
        FilterScope::Active => list_declarations(
            storage,
            &ListOptions {
                show_all: false,
                status: None,
            },
        ),
        FilterScope::PendingPayment => {
            let mut decls = list_declarations(
                storage,
                &ListOptions {
                    show_all: true,
                    status: None,
                },
            );
            decls.retain(|d| {
                d.status == DeclarationStatus::Submitted || d.status == DeclarationStatus::Pending
            });
            decls
        }
    }
}

fn check_holiday_warning(config: &UserConfig) -> Option<String> {
    let mut calendar = HolidayCalendar::load_embedded();
    let data_dir = app_config::get_effective_data_dir_path(config);
    calendar.merge_file(&data_dir);
    let year = chrono::Local::now().year();
    if calendar.is_year_loaded(year) {
        None
    } else {
        Some(format!(
            "Holiday calendar data does not cover {year}. \
             Exchange rate lookback near holidays may be inaccurate. \
             Click Sync or update the app."
        ))
    }
}
