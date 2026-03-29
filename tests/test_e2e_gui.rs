#![cfg(feature = "gui")]

use std::collections::HashSet;

use chrono::NaiveDate;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use rust_decimal_macros::dec;

use ibkr_porez::gui::app::{self, App, BulkAction, FilterScope, SortColumn};
use ibkr_porez::gui::config_dialog::ConfigDialog;
use ibkr_porez::models::{
    Currency, Declaration, DeclarationStatus, DeclarationType, Transaction, TransactionType,
    UserConfig,
};
use ibkr_porez::storage::Storage;

fn ts(id: &str) -> chrono::NaiveDateTime {
    let n: u32 = id.bytes().map(u32::from).sum();
    chrono::NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        chrono::NaiveTime::from_hms_opt(0, 0, n % 60).unwrap(),
    )
}

fn make_decl(id: &str, status: DeclarationStatus) -> Declaration {
    Declaration {
        declaration_id: id.to_string(),
        r#type: DeclarationType::Ppo,
        status,
        period_start: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2024, 6, 30).unwrap(),
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

fn make_transaction(id: &str, date: NaiveDate) -> Transaction {
    Transaction {
        transaction_id: id.to_string(),
        date,
        r#type: TransactionType::Dividend,
        symbol: "TEST".to_string(),
        description: "test dividend".to_string(),
        quantity: dec!(0),
        price: dec!(0),
        amount: dec!(10),
        currency: Currency::USD,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
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

fn setup_app(decls: Vec<Declaration>, transactions: Vec<Transaction>) -> (App, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    for d in &decls {
        storage.save_declaration(d).unwrap();
    }
    if !transactions.is_empty() {
        storage.save_transactions(&transactions).unwrap();
    }
    let show_import_hint = storage.get_last_transaction_date().is_none();
    let filtered = app::load_filtered(&storage, FilterScope::Active);

    let config = valid_test_config(&tmp);
    let config_file = tmp.path().join("config.json");
    let json = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&config_file, json).unwrap();

    let app = App {
        config,
        config_file,
        storage,
        declarations: filtered,
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
        show_import_hint,
        confirm_discard_config: false,
    };
    (app, tmp)
}

fn setup_app_invalid_config(
    decls: Vec<Declaration>,
    transactions: Vec<Transaction>,
) -> (App, tempfile::TempDir) {
    let (mut app, tmp) = setup_app(decls, transactions);
    app.config.ibkr_token = String::new();
    let json = serde_json::to_string_pretty(&app.config).unwrap();
    std::fs::write(&app.config_file, json).unwrap();
    (app, tmp)
}

fn harness_for(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1024.0, 768.0))
        .build_state(|ctx, app: &mut App| app.render(ctx), app)
}

// ── Table and layout ─────────────────────────────────────────

#[test]
fn table_shows_seeded_declarations() {
    let (app, _tmp) = setup_app(
        vec![
            make_decl("decl-a", DeclarationStatus::Draft),
            make_decl("decl-b", DeclarationStatus::Draft),
        ],
        vec![],
    );
    let harness = harness_for(app);

    assert!(
        harness.query_by_label_contains("decl-a").is_some(),
        "decl-a should appear in the accessibility tree"
    );
    assert!(
        harness.query_by_label_contains("decl-b").is_some(),
        "decl-b should appear in the accessibility tree"
    );
}

#[test]
fn empty_db_shows_import_hint() {
    let (app, _tmp) = setup_app(vec![], vec![]);
    let harness = harness_for(app);

    assert!(
        harness
            .query_by_label_contains("Transaction history is empty")
            .is_some(),
        "import hint should appear when no transactions exist"
    );
}

#[test]
fn non_empty_db_hides_import_hint() {
    let tx = make_transaction("tx-1", NaiveDate::from_ymd_opt(2024, 4, 14).unwrap());
    let (app, _tmp) = setup_app(vec![], vec![tx]);
    let harness = harness_for(app);

    assert!(
        harness
            .query_by_label_contains("Transaction history is empty")
            .is_none(),
        "import hint must be hidden when transactions exist"
    );
}

// ── Toolbar buttons ──────────────────────────────────────────

#[test]
fn config_button_opens_dialog() {
    let (app, _tmp) = setup_app(vec![], vec![]);
    let mut harness = harness_for(app);

    assert!(harness.state().config_dialog.is_none());

    harness.get_by_label_contains("Config").click();
    harness.run();

    assert!(
        harness.state().config_dialog.is_some(),
        "clicking Config button should open config dialog"
    );
}

#[test]
fn sync_button_shows_config_error() {
    let (app, _tmp) = setup_app_invalid_config(vec![], vec![]);
    let mut harness = harness_for(app);

    harness.get_by_label_contains("Sync").click();
    harness.run();

    assert!(
        harness.state().error_dialog.is_some(),
        "sync with invalid config should show error dialog"
    );
}

// ── Filter bar ───────────────────────────────────────────────

#[test]
fn filter_all_shows_finalized() {
    let (app, _tmp) = setup_app(
        vec![
            make_decl("active-decl", DeclarationStatus::Draft),
            make_decl("final-decl", DeclarationStatus::Finalized),
        ],
        vec![],
    );
    let mut harness = harness_for(app);

    harness.get_by_label("All").click();
    harness.run();

    assert!(
        harness.query_by_label_contains("final-decl").is_some(),
        "All filter should show finalized declarations"
    );
    assert!(
        harness.query_by_label_contains("active-decl").is_some(),
        "All filter should also show active declarations"
    );
}

#[test]
fn filter_active_hides_finalized() {
    let (app, _tmp) = setup_app(
        vec![
            make_decl("active-decl", DeclarationStatus::Draft),
            make_decl("final-decl", DeclarationStatus::Finalized),
        ],
        vec![],
    );
    let harness = harness_for(app);

    assert!(
        harness.query_by_label_contains("final-decl").is_none(),
        "Active filter (default) should hide finalized declarations"
    );
    assert!(
        harness.query_by_label_contains("active-decl").is_some(),
        "Active filter should show draft declarations"
    );
}

// ── Dialog open/close via ESC ────────────────────────────────

#[test]
fn esc_closes_config_dialog() {
    let (app, _tmp) = setup_app(vec![], vec![]);
    let mut harness = harness_for(app);

    harness.state_mut().config_dialog = Some(ConfigDialog::new(&harness.state().config.clone()));
    harness.run();

    harness.press_key(egui::Key::Escape);
    harness.run();

    assert!(
        harness.state().config_dialog.is_none(),
        "ESC should close unmodified config dialog"
    );
}

#[test]
fn esc_on_changed_config_shows_discard_confirm() {
    let (app, _tmp) = setup_app(vec![], vec![]);
    let mut harness = harness_for(app);

    let config = harness.state().config.clone();
    harness.state_mut().config_dialog = Some(ConfigDialog::new(&config));
    harness
        .state_mut()
        .config_dialog
        .as_mut()
        .unwrap()
        .config
        .ibkr_token = "changed".into();
    harness.run();

    harness.press_key(egui::Key::Escape);
    harness.run();

    assert!(
        harness.state().confirm_discard_config,
        "ESC on modified config dialog should show discard confirmation"
    );
}

#[test]
fn esc_closes_error_dialog() {
    let (app, _tmp) = setup_app(vec![], vec![]);
    let mut harness = harness_for(app);

    harness.state_mut().error_dialog = Some("test error".into());
    harness.run();

    harness.press_key(egui::Key::Escape);
    harness.run();

    assert!(
        harness.state().error_dialog.is_none(),
        "ESC should close error dialog"
    );
}

// ── Select and bulk action ───────────────────────────────────

#[test]
fn select_all_and_apply_submit() {
    let (app, _tmp) = setup_app(
        vec![
            make_decl("sub-1", DeclarationStatus::Draft),
            make_decl("sub-2", DeclarationStatus::Draft),
        ],
        vec![],
    );
    let mut harness = harness_for(app);

    harness.get_by_label("Select all").click();
    harness.run();

    harness.get_by_label("Apply").click();
    harness.run();

    let state = harness.state();
    for d in &state.declarations {
        assert_eq!(
            d.status,
            DeclarationStatus::Submitted,
            "declaration {} should be Submitted after bulk submit",
            d.declaration_id
        );
    }
}

// ── Status bar ───────────────────────────────────────────────

#[test]
fn status_bar_hint_text() {
    let (app, _tmp) = setup_app(vec![], vec![]);
    let harness = harness_for(app);

    assert!(
        harness.query_by_label_contains("Double-click").is_some(),
        "status bar hint should be visible"
    );
}
