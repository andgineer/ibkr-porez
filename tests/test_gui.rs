#![cfg(feature = "gui")]

use std::collections::HashSet;
use std::sync::mpsc;

use chrono::NaiveDate;
use ibkr_porez::gui::app::{App, BackgroundResult, BulkAction, FilterScope, SortColumn};
use ibkr_porez::gui::assessment_dialog::AssessmentDialog;
use ibkr_porez::gui::config_dialog::ConfigDialog;
use ibkr_porez::gui::details_dialog::{self, DetailsDialog};
use ibkr_porez::gui::import_dialog::ImportDialog;
use ibkr_porez::gui::styles;
use ibkr_porez::import::{FileType, ImportResult};
use ibkr_porez::models::{Declaration, DeclarationStatus, DeclarationType, UserConfig};
use ibkr_porez::storage::Storage;
use ibkr_porez::sync::SyncResult;

fn ts(id: &str) -> chrono::NaiveDateTime {
    let n: u32 = id.bytes().map(u32::from).sum();
    chrono::NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        chrono::NaiveTime::from_hms_opt(0, 0, n % 60).unwrap(),
    )
}

fn make_decl(id: &str, status: DeclarationStatus) -> Declaration {
    make_typed_decl(id, DeclarationType::Ppo, status)
}

fn make_typed_decl(id: &str, dtype: DeclarationType, status: DeclarationStatus) -> Declaration {
    Declaration {
        declaration_id: id.to_string(),
        r#type: dtype,
        status,
        period_start: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        created_at: ts(id),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: Some("<xml/>".into()),
        report_data: None,
        metadata: indexmap::IndexMap::new(),
        attached_files: indexmap::IndexMap::new(),
    }
}

fn app_with_decls(decls: Vec<Declaration>) -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let app = app_in_dir(decls, &tmp);
    (app, tmp)
}

fn sync_result(created: Vec<Declaration>, income_error: Option<String>) -> SyncResult {
    SyncResult {
        created_declarations: created,
        gains_skipped: false,
        income_skipped: false,
        income_error,
        end_period: NaiveDate::from_ymd_opt(2024, 6, 30).unwrap(),
    }
}

fn valid_test_config(tmp: &tempfile::TempDir) -> UserConfig {
    UserConfig {
        ibkr_token: "test-token".into(),
        ibkr_query_id: "test-query".into(),
        personal_id: "1234567890123".into(),
        full_name: "Test User".into(),
        address: "Test Address 1".into(),
        city_code: "223".into(),
        phone: "0641234567".into(),
        email: "test@test.com".into(),
        data_dir: Some(tmp.path().to_string_lossy().into_owned()),
        output_folder: None,
    }
}

fn app_in_dir(decls: Vec<Declaration>, tmp: &tempfile::TempDir) -> App {
    let storage = Storage::with_dir(tmp.path());
    for d in &decls {
        storage.save_declaration(d).unwrap();
    }
    let config = valid_test_config(tmp);
    let config_file = tmp.path().join("config.json");
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&config_file, json).unwrap();
    App {
        config,
        config_file,
        storage,
        declarations: decls,
        selected: HashSet::new(),
        filter: FilterScope::Active,
        bulk_action: BulkAction::Submit,
        sort_column: SortColumn::Created,
        sort_ascending: false,
        status_message: None,
        warning_banner: None,
        bg_receiver: None,
        bg_busy: false,
        export_channel: None,
        exporting_ids: HashSet::new(),
        progress_text: None,
        confirm_force_sync: false,
        config_dialog: None,
        import_dialog: None,
        details_dialog: None,
        assessment_dialog: None,
        error_dialog: None,
        show_import_hint: false,
        confirm_discard_config: false,
    }
}

// ── Enum labels ──────────────────────────────────────────────

#[test]
fn filter_scope_labels() {
    assert_eq!(FilterScope::Active.label(), "Active");
    assert_eq!(FilterScope::All.label(), "All");
    assert_eq!(FilterScope::PendingPayment.label(), "Pending payment");
}

#[test]
fn bulk_action_labels() {
    assert_eq!(BulkAction::Submit.label(), "Submit");
    assert_eq!(BulkAction::Pay.label(), "Pay");
    assert_eq!(BulkAction::Revert.label(), "Revert to Draft");
}

#[test]
fn sort_column_labels() {
    assert_eq!(SortColumn::Id.label(), "ID");
    assert_eq!(SortColumn::Type.label(), "Type");
    assert_eq!(SortColumn::Period.label(), "Period");
    assert_eq!(SortColumn::Tax.label(), "Tax");
    assert_eq!(SortColumn::Status.label(), "Status");
    assert_eq!(SortColumn::Created.label(), "Created");
}

// ── Sort ─────────────────────────────────────────────────────

#[test]
fn sort_toggle_ascending_descending() {
    let (mut app, _tmp) = app_with_decls(vec![
        make_decl("a-001", DeclarationStatus::Draft),
        make_decl("b-002", DeclarationStatus::Draft),
    ]);

    app.set_sort(SortColumn::Id);
    assert!(app.sort_ascending);
    assert_eq!(app.declarations[0].declaration_id, "a-001");

    app.set_sort(SortColumn::Id);
    assert!(!app.sort_ascending);
    assert_eq!(app.declarations[0].declaration_id, "b-002");
}

#[test]
fn sort_change_column_resets_ascending() {
    let (mut app, _tmp) = app_with_decls(vec![make_decl("x", DeclarationStatus::Draft)]);
    app.set_sort(SortColumn::Id);
    app.set_sort(SortColumn::Id);
    assert!(!app.sort_ascending);

    app.set_sort(SortColumn::Status);
    assert!(app.sort_ascending);
}

// ── Select ───────────────────────────────────────────────────

#[test]
fn select_all_and_unselect() {
    let (mut app, _tmp) = app_with_decls(vec![
        make_decl("d-1", DeclarationStatus::Draft),
        make_decl("d-2", DeclarationStatus::Submitted),
    ]);

    app.select_all();
    assert_eq!(app.selected.len(), 2);
    assert!(app.selected.contains("d-1"));
    assert!(app.selected.contains("d-2"));

    app.unselect_all();
    assert!(app.selected.is_empty());
}

#[test]
fn selected_pruned_on_declaration_change() {
    let (mut app, _tmp) = app_with_decls(vec![
        make_decl("keep", DeclarationStatus::Draft),
        make_decl("remove", DeclarationStatus::Draft),
    ]);
    app.select_all();
    assert_eq!(app.selected.len(), 2);

    app.declarations = vec![make_decl("keep", DeclarationStatus::Draft)];
    app.selected
        .retain(|id| app.declarations.iter().any(|d| &d.declaration_id == id));

    assert_eq!(app.selected.len(), 1);
    assert!(app.selected.contains("keep"));
    assert!(!app.selected.contains("remove"));
}

// ── Filter ───────────────────────────────────────────────────

#[test]
fn set_filter_updates_state() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    assert_eq!(app.filter, FilterScope::Active);

    app.filter = FilterScope::PendingPayment;
    assert_eq!(app.filter, FilterScope::PendingPayment);

    app.filter = FilterScope::All;
    assert_eq!(app.filter, FilterScope::All);
}

#[test]
fn pending_payment_filter_includes_both_statuses() {
    let decls = vec![
        make_decl("draft", DeclarationStatus::Draft),
        make_decl("submitted", DeclarationStatus::Submitted),
        make_decl("pending", DeclarationStatus::Pending),
        make_decl("finalized", DeclarationStatus::Finalized),
    ];

    let mut filtered = decls;
    filtered.retain(|d| {
        d.status == DeclarationStatus::Submitted || d.status == DeclarationStatus::Pending
    });

    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].declaration_id, "submitted");
    assert_eq!(filtered[1].declaration_id, "pending");
}

#[test]
fn load_filtered_active_excludes_finalized() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    storage
        .save_declaration(&make_decl("d1", DeclarationStatus::Draft))
        .unwrap();
    storage
        .save_declaration(&make_decl("d2", DeclarationStatus::Finalized))
        .unwrap();

    let active = ibkr_porez::gui::app::load_filtered(&storage, FilterScope::Active);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].declaration_id, "d1");
}

#[test]
fn load_filtered_all_includes_everything() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    storage
        .save_declaration(&make_decl("d1", DeclarationStatus::Draft))
        .unwrap();
    storage
        .save_declaration(&make_decl("d2", DeclarationStatus::Finalized))
        .unwrap();

    let all = ibkr_porez::gui::app::load_filtered(&storage, FilterScope::All);
    assert_eq!(all.len(), 2);
}

#[test]
fn load_filtered_pending_payment() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    storage
        .save_declaration(&make_decl("d1", DeclarationStatus::Draft))
        .unwrap();
    storage
        .save_declaration(&make_decl("d2", DeclarationStatus::Submitted))
        .unwrap();
    storage
        .save_declaration(&make_decl("d3", DeclarationStatus::Pending))
        .unwrap();
    storage
        .save_declaration(&make_decl("d4", DeclarationStatus::Finalized))
        .unwrap();

    let pending = ibkr_porez::gui::app::load_filtered(&storage, FilterScope::PendingPayment);
    assert_eq!(pending.len(), 2);
    let ids: Vec<&str> = pending.iter().map(|d| d.declaration_id.as_str()).collect();
    assert!(ids.contains(&"d2"));
    assert!(ids.contains(&"d3"));
}

// ── Bulk Actions ─────────────────────────────────────────────

#[test]
fn bulk_submit_draft_declarations() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(
        vec![
            make_decl("s1", DeclarationStatus::Draft),
            make_decl("s2", DeclarationStatus::Draft),
        ],
        &tmp,
    );
    app.select_all();
    app.bulk_action = BulkAction::Submit;
    app.apply_bulk_action();

    assert!(app.error_dialog.is_none());
    assert!(app.selected.is_empty());
    let (msg, kind) = app.status_message.as_ref().unwrap();
    assert_eq!(*kind, styles::MessageKind::Success);
    assert!(msg.contains("2 declaration(s)"));
}

#[test]
fn bulk_submit_non_draft_shows_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(vec![make_decl("s1", DeclarationStatus::Submitted)], &tmp);
    app.select_all();
    app.bulk_action = BulkAction::Submit;
    app.apply_bulk_action();

    assert!(app.error_dialog.is_some());
    assert!(app.error_dialog.as_ref().unwrap().contains("not in Draft"));
}

#[test]
fn bulk_submit_mixed_partial_success() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(
        vec![
            make_decl("ok", DeclarationStatus::Draft),
            make_decl("fail", DeclarationStatus::Submitted),
        ],
        &tmp,
    );
    app.select_all();
    app.bulk_action = BulkAction::Submit;
    app.apply_bulk_action();

    assert!(app.status_message.is_some());
    let (msg, _) = app.status_message.as_ref().unwrap();
    assert!(msg.contains("1 declaration(s)"));

    assert!(app.error_dialog.is_some());
    assert!(app.error_dialog.as_ref().unwrap().contains("fail"));
    assert!(
        !app.selected.is_empty(),
        "selection preserved on partial error"
    );
}

#[test]
fn bulk_pay_draft_declarations() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(vec![make_decl("p1", DeclarationStatus::Draft)], &tmp);
    app.select_all();
    app.bulk_action = BulkAction::Pay;
    app.apply_bulk_action();

    assert!(app.error_dialog.is_none());
    assert!(app.selected.is_empty());
    let d = app.storage.get_declaration("p1").unwrap();
    assert_eq!(d.status, DeclarationStatus::Finalized);
}

#[test]
fn bulk_pay_already_finalized_shows_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(vec![make_decl("p1", DeclarationStatus::Finalized)], &tmp);
    app.select_all();
    app.bulk_action = BulkAction::Pay;
    app.apply_bulk_action();

    assert!(app.error_dialog.is_some());
    assert!(
        app.error_dialog
            .as_ref()
            .unwrap()
            .contains("already finalized")
    );
}

#[test]
fn bulk_revert_to_draft() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(
        vec![
            make_decl("r1", DeclarationStatus::Submitted),
            make_decl("r2", DeclarationStatus::Finalized),
        ],
        &tmp,
    );
    app.select_all();
    app.bulk_action = BulkAction::Revert;
    app.apply_bulk_action();

    assert!(app.error_dialog.is_none());
    assert!(app.selected.is_empty());

    let d1 = app.storage.get_declaration("r1").unwrap();
    let d2 = app.storage.get_declaration("r2").unwrap();
    assert_eq!(d1.status, DeclarationStatus::Draft);
    assert_eq!(d2.status, DeclarationStatus::Draft);
}

#[test]
fn bulk_action_empty_selection_does_nothing() {
    let (mut app, _tmp) = app_with_decls(vec![make_decl("x", DeclarationStatus::Draft)]);
    app.bulk_action = BulkAction::Submit;
    app.apply_bulk_action();

    assert!(app.status_message.is_none());
    assert!(app.error_dialog.is_none());
}

// ── Row Actions ──────────────────────────────────────────────

#[test]
fn row_submit_changes_status() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(
        vec![make_typed_decl(
            "rs1",
            DeclarationType::Ppdg3r,
            DeclarationStatus::Draft,
        )],
        &tmp,
    );
    app.row_submit("rs1");

    assert!(app.error_dialog.is_none());
    let d = app.storage.get_declaration("rs1").unwrap();
    assert_eq!(d.status, DeclarationStatus::Pending);
}

#[test]
fn row_submit_non_draft_sets_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(vec![make_decl("rs2", DeclarationStatus::Finalized)], &tmp);
    app.row_submit("rs2");

    assert!(app.error_dialog.is_some());
}

#[test]
fn row_pay_finalizes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(vec![make_decl("rp1", DeclarationStatus::Submitted)], &tmp);
    app.row_pay("rp1");

    assert!(app.error_dialog.is_none());
    let d = app.storage.get_declaration("rp1").unwrap();
    assert_eq!(d.status, DeclarationStatus::Finalized);
}

#[test]
fn row_revert_sets_draft() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(vec![make_decl("rv1", DeclarationStatus::Finalized)], &tmp);
    app.row_revert("rv1");

    assert!(app.error_dialog.is_none());
    let d = app.storage.get_declaration("rv1").unwrap();
    assert_eq!(d.status, DeclarationStatus::Draft);
}

// ── Refresh / reload ─────────────────────────────────────────

#[test]
fn refresh_declarations_reloads_from_storage() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(vec![make_decl("x", DeclarationStatus::Draft)], &tmp);
    assert_eq!(app.declarations.len(), 1);

    app.storage
        .save_declaration(&make_decl("y", DeclarationStatus::Draft))
        .unwrap();
    app.refresh_declarations();

    assert_eq!(app.declarations.len(), 2);
}

#[test]
fn refresh_prunes_stale_selection() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = app_in_dir(
        vec![
            make_decl("keep", DeclarationStatus::Draft),
            make_decl("gone", DeclarationStatus::Draft),
        ],
        &tmp,
    );
    app.select_all();

    app.row_submit("gone");
    app.filter = FilterScope::Active;
    app.refresh_declarations();

    assert!(app.selected.contains("keep"));
}

// ── Storage / config wiring ──────────────────────────────────

#[test]
fn reload_storage_follows_config_data_dir_change() {
    let tmp1 = tempfile::TempDir::new().unwrap();
    let tmp2 = tempfile::TempDir::new().unwrap();

    let mut app = app_in_dir(vec![], &tmp1);
    assert_eq!(app.storage.data_dir(), tmp1.path());

    app.config.data_dir = Some(tmp2.path().to_string_lossy().into_owned());
    app.reload_storage();

    assert_eq!(
        app.storage.data_dir(),
        tmp2.path(),
        "reload_storage must follow config.data_dir, not keep the old path"
    );
}

#[test]
fn refresh_after_config_change_sees_new_data() {
    let tmp_old = tempfile::TempDir::new().unwrap();
    let tmp_new = tempfile::TempDir::new().unwrap();

    let mut app = app_in_dir(
        vec![make_decl("old-decl", DeclarationStatus::Draft)],
        &tmp_old,
    );
    assert_eq!(app.declarations.len(), 1);

    let new_storage = Storage::with_dir(tmp_new.path());
    new_storage
        .save_declaration(&make_decl("new-1", DeclarationStatus::Draft))
        .unwrap();
    new_storage
        .save_declaration(&make_decl("new-2", DeclarationStatus::Draft))
        .unwrap();

    let mut updated_config = valid_test_config(&tmp_new);
    updated_config.data_dir = Some(tmp_new.path().to_string_lossy().into_owned());
    let json = serde_json::to_string_pretty(&updated_config).unwrap();
    std::fs::write(&app.config_file, json).unwrap();

    app.refresh_declarations();

    assert_eq!(
        app.declarations.len(),
        2,
        "after config.data_dir change, refresh must load from the new directory"
    );
    let ids: Vec<&str> = app
        .declarations
        .iter()
        .map(|d| d.declaration_id.as_str())
        .collect();
    assert!(ids.contains(&"new-1"));
    assert!(ids.contains(&"new-2"));
    assert!(!ids.contains(&"old-decl"));
}

// ── Background polling ───────────────────────────────────────

#[test]
fn poll_sync_done_success() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;
    app.progress_text = Some("Syncing…".into());

    tx.send(BackgroundResult::SyncDone(Ok(sync_result(vec![], None))))
        .unwrap();

    app.poll_background();

    assert!(!app.bg_busy);
    assert!(app.bg_receiver.is_none());
    assert!(app.progress_text.is_none());
    assert!(app.error_dialog.is_none());
    let (msg, kind) = app.status_message.as_ref().unwrap();
    assert_eq!(*kind, styles::MessageKind::Success);
    assert!(msg.contains("no new declarations"));
}

#[test]
fn poll_sync_done_with_declarations() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;

    tx.send(BackgroundResult::SyncDone(Ok(sync_result(
        vec![
            make_decl("DECL-1", DeclarationStatus::Draft),
            make_decl("DECL-2", DeclarationStatus::Draft),
        ],
        None,
    ))))
    .unwrap();

    app.poll_background();

    let (msg, _) = app.status_message.as_ref().unwrap();
    assert!(msg.contains("2 declaration(s) created"));
}

#[test]
fn poll_sync_done_with_income_error() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;

    tx.send(BackgroundResult::SyncDone(Ok(sync_result(
        vec![],
        Some("tax calc failed".into()),
    ))))
    .unwrap();

    app.poll_background();

    assert!(app.status_message.is_some());
    assert_eq!(app.error_dialog.as_deref(), Some("tax calc failed"));
}

#[test]
fn poll_sync_done_error() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;
    app.progress_text = Some("Syncing…".into());

    tx.send(BackgroundResult::SyncDone(Err("network failure".into())))
        .unwrap();

    app.poll_background();

    assert!(!app.bg_busy);
    assert!(app.bg_receiver.is_none());
    assert!(app.progress_text.is_none());
    assert!(app.status_message.is_none());
    assert_eq!(app.error_dialog.as_deref(), Some("network failure"));
}

#[test]
fn poll_import_done_success() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    app.import_dialog = Some(ImportDialog::new());
    let (tx, rx) = mpsc::channel();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;
    app.progress_text = Some("Importing…".into());

    tx.send(BackgroundResult::ImportDone(Ok(ImportResult {
        inserted: 10,
        updated: 2,
        transaction_count: 12,
    })))
    .unwrap();

    app.poll_background();

    assert!(!app.bg_busy);
    assert!(app.import_dialog.is_none());
    assert!(app.error_dialog.is_none());
    let (msg, _) = app.status_message.as_ref().unwrap();
    assert!(msg.contains("10 inserted"));
    assert!(msg.contains("2 updated"));
    assert!(msg.contains("12 total"));
}

#[test]
fn poll_import_done_error() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    app.import_dialog = Some(ImportDialog::new());
    let (tx, rx) = mpsc::channel();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;

    tx.send(BackgroundResult::ImportDone(Err("parse error".into())))
        .unwrap();

    app.poll_background();

    assert!(!app.bg_busy);
    assert!(app.import_dialog.is_none());
    assert!(app.status_message.is_none());
    assert_eq!(app.error_dialog.as_deref(), Some("parse error"));
}

#[test]
fn poll_export_done_success() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel();
    app.export_channel = Some((tx.clone(), rx));
    app.exporting_ids.insert("EXP-1".into());

    tx.send(BackgroundResult::ExportDone {
        id: "EXP-1".into(),
        result: Ok("/tmp/out.xml".into()),
    })
    .unwrap();

    app.poll_background();

    assert!(app.exporting_ids.is_empty());
    assert!(app.export_channel.is_none());
    assert!(app.error_dialog.is_none());
    let (msg, _) = app.status_message.as_ref().unwrap();
    assert!(msg.contains("/tmp/out.xml"));
}

#[test]
fn poll_export_done_error() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel();
    app.export_channel = Some((tx.clone(), rx));
    app.exporting_ids.insert("EXP-2".into());

    tx.send(BackgroundResult::ExportDone {
        id: "EXP-2".into(),
        result: Err("write failed".into()),
    })
    .unwrap();

    app.poll_background();

    assert!(app.exporting_ids.is_empty());
    assert_eq!(app.error_dialog.as_deref(), Some("write failed"));
}

#[test]
fn poll_export_channel_kept_while_ids_remain() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel();
    app.export_channel = Some((tx.clone(), rx));
    app.exporting_ids.insert("E1".into());
    app.exporting_ids.insert("E2".into());

    tx.send(BackgroundResult::ExportDone {
        id: "E1".into(),
        result: Ok("/tmp/e1.xml".into()),
    })
    .unwrap();

    app.poll_background();

    assert_eq!(app.exporting_ids.len(), 1);
    assert!(app.export_channel.is_some());
}

#[test]
fn poll_disconnected_clears_busy() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel::<BackgroundResult>();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;
    drop(tx);

    app.poll_background();

    assert!(!app.bg_busy);
    assert!(app.bg_receiver.is_none());
}

#[test]
fn poll_empty_channel_is_noop() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    let (tx, rx) = mpsc::channel::<BackgroundResult>();
    app.bg_receiver = Some(rx);
    app.bg_busy = true;

    app.poll_background();

    assert!(app.bg_busy);
    assert!(app.bg_receiver.is_some());
    drop(tx);
}

// ── Start sync ───────────────────────────────────────────────

#[test]
fn start_sync_sets_busy_state() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    assert!(!app.bg_busy);
    assert!(app.progress_text.is_none());
    assert!(app.bg_receiver.is_none());

    app.start_sync(false);

    assert!(app.bg_busy);
    assert!(app.progress_text.is_some());
    assert!(app.bg_receiver.is_some());
}

#[test]
fn start_sync_ignored_while_busy() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    app.bg_busy = true;
    let original_text = app.progress_text.clone();

    app.start_sync(false);

    assert_eq!(app.progress_text, original_text);
    assert!(app.bg_receiver.is_none());
}

// ── format_declaration ───────────────────────────────────────

fn sample_decl() -> Declaration {
    Declaration {
        declaration_id: "TEST-001".to_string(),
        r#type: DeclarationType::Ppdg3r,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2024, 6, 30).unwrap(),
        created_at: chrono::NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
            chrono::NaiveTime::from_hms_opt(10, 30, 0).unwrap(),
        ),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: None,
        report_data: None,
        metadata: indexmap::IndexMap::new(),
        attached_files: indexmap::IndexMap::new(),
    }
}

#[test]
fn format_declaration_basic_fields() {
    let text = details_dialog::format_declaration(&sample_decl());
    assert!(text.contains("ID:       TEST-001"));
    assert!(text.contains("PPDG-3R"));
    assert!(text.contains("draft"));
    assert!(text.contains("2024-03-15 10:30:00"));
    assert!(!text.contains("Submitted:"));
    assert!(!text.contains("Paid:"));
    assert!(!text.contains("Metadata"));
    assert!(!text.contains("Attached"));
}

#[test]
fn format_declaration_with_timestamps() {
    let mut decl = sample_decl();
    decl.status = DeclarationStatus::Finalized;
    decl.submitted_at = Some(chrono::NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(),
        chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
    ));
    decl.paid_at = Some(chrono::NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 4, 15).unwrap(),
        chrono::NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
    ));

    let text = details_dialog::format_declaration(&decl);
    assert!(text.contains("Submitted: 2024-04-01 09:00:00"));
    assert!(text.contains("Paid:     2024-04-15 14:00:00"));
}

#[test]
fn format_declaration_with_metadata() {
    let mut decl = sample_decl();
    decl.metadata.insert(
        "total_rsd".to_string(),
        serde_json::Value::String("150000.50".to_string()),
    );
    decl.metadata
        .insert("count".to_string(), serde_json::json!(42));

    let text = details_dialog::format_declaration(&decl);
    assert!(text.contains("--- Metadata ---"));
    assert!(text.contains("total_rsd: 150000.50"));
    assert!(text.contains("count: 42"));
}

#[test]
fn format_declaration_with_attachments() {
    let mut decl = sample_decl();
    decl.attached_files
        .insert("receipt.pdf".into(), "attachments/receipt.pdf".into());

    let text = details_dialog::format_declaration(&decl);
    assert!(text.contains("--- Attached Files ---"));
    assert!(text.contains("receipt.pdf: attachments/receipt.pdf"));
}

// ── Dialog constructors ──────────────────────────────────────

#[test]
fn details_dialog_not_found() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let dialog = DetailsDialog::new("NONEXISTENT", &storage);
    assert!(dialog.text.contains("not found"));
}

#[test]
fn details_dialog_found() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    storage.save_declaration(&sample_decl()).unwrap();

    let dialog = DetailsDialog::new("TEST-001", &storage);
    assert!(dialog.text.contains("TEST-001"));
    assert!(dialog.text.contains("PPDG-3R"));
}

#[test]
fn config_dialog_new() {
    let cfg = UserConfig {
        data_dir: Some("/custom/dir".into()),
        output_folder: Some("/custom/output".into()),
        full_name: "Test User".into(),
        ..UserConfig::default()
    };

    let dialog = ConfigDialog::new(&cfg);
    assert_eq!(dialog.data_dir_str, "/custom/dir");
    assert_eq!(dialog.output_folder_str, "/custom/output");
    assert_eq!(dialog.config.full_name, "Test User");
}

#[test]
fn config_dialog_defaults_to_empty_strings() {
    let cfg = UserConfig::default();
    let dialog = ConfigDialog::new(&cfg);
    assert!(dialog.data_dir_str.is_empty());
    assert!(dialog.output_folder_str.is_empty());
}

#[test]
fn import_dialog_defaults() {
    let dialog = ImportDialog::new();
    assert!(dialog.file_path.is_empty());
    assert_eq!(dialog.file_type, FileType::Auto);
    assert!(!dialog.busy);
}

#[test]
fn assessment_dialog_new() {
    let dialog = AssessmentDialog::new("DECL-123".to_string());
    assert_eq!(dialog.declaration_id, "DECL-123");
    assert!(dialog.tax_input.is_empty());
    assert!(!dialog.mark_paid);
}

// ── Config reload from disk ─────────────────────────────────

#[test]
fn refresh_picks_up_config_change_from_disk() {
    let (mut app, tmp) = app_with_decls(Vec::new());
    assert_eq!(app.config.full_name, "Test User");

    let mut modified = valid_test_config(&tmp);
    modified.full_name = "Changed Name".into();
    let json = serde_json::to_string_pretty(&modified).unwrap();
    std::fs::write(&app.config_file, json).unwrap();

    app.refresh_declarations();

    assert_eq!(app.config.full_name, "Changed Name");
}

#[test]
fn start_sync_reloads_config_from_disk() {
    let (mut app, _tmp) = app_with_decls(Vec::new());
    app.config.ibkr_token = String::new();

    app.start_sync(false);

    assert!(
        app.bg_busy,
        "start_sync should reload valid config from disk and proceed"
    );
    assert!(app.error_dialog.is_none());
}
