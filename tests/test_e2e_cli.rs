use std::path::PathBuf;

use assert_cmd::Command;
use chrono::{Local, NaiveDate};
use ibkr_porez::declaration_manager::DeclarationManager;
use ibkr_porez::models::{
    Currency, Declaration, DeclarationStatus, DeclarationType, Transaction, TransactionType,
};
use ibkr_porez::storage::Storage;
use indexmap::IndexMap;
use predicates::prelude::*;
use rust_decimal::Decimal;
use std::str::FromStr;

// ── Helpers ─────────────────────────────────────────────────

fn cmd() -> Command {
    Command::cargo_bin("ibkr-porez").unwrap()
}

fn setup_env() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();

    let config = serde_json::json!({ "data_dir": data_dir.to_str().unwrap() });
    std::fs::write(tmp.path().join("config.json"), config.to_string()).unwrap();

    (tmp, data_dir)
}

fn make_declaration(
    storage: &Storage,
    id: &str,
    decl_type: DeclarationType,
    status: DeclarationStatus,
    tax_due: Option<&str>,
) {
    let mut metadata = IndexMap::new();
    if let Some(tax) = tax_due {
        metadata.insert("tax_due_rsd".into(), serde_json::Value::String(tax.into()));
    }
    let decl = Declaration {
        declaration_id: id.to_string(),
        r#type: decl_type,
        status,
        period_start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
        created_at: Local::now().naive_local(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: Some("<xml/>".into()),
        report_data: None,
        metadata,
        attached_files: IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();
}

fn make_draft(storage: &Storage, id: &str) {
    make_declaration(
        storage,
        id,
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
        None,
    );
}

fn make_trade(
    id: &str,
    date: NaiveDate,
    symbol: &str,
    qty: &str,
    price: &str,
    amount: &str,
    open_date: Option<NaiveDate>,
    open_price: Option<&str>,
    rate: &str,
) -> Transaction {
    let amt = Decimal::from_str(amount).unwrap();
    let r = Decimal::from_str(rate).unwrap();
    Transaction {
        transaction_id: id.into(),
        date,
        r#type: TransactionType::Trade,
        symbol: symbol.into(),
        description: format!("Trade {symbol}"),
        quantity: Decimal::from_str(qty).unwrap(),
        price: Decimal::from_str(price).unwrap(),
        amount: amt,
        currency: Currency::USD,
        open_date,
        open_price: open_price.map(|p| Decimal::from_str(p).unwrap()),
        exchange_rate: Some(r),
        amount_rsd: Some(amt * r),
    }
}

fn make_dividend(id: &str, date: NaiveDate, symbol: &str, amount: &str, rate: &str) -> Transaction {
    let amt = Decimal::from_str(amount).unwrap();
    let r = Decimal::from_str(rate).unwrap();
    Transaction {
        transaction_id: id.into(),
        date,
        r#type: TransactionType::Dividend,
        symbol: symbol.into(),
        description: format!("Dividend {symbol}"),
        quantity: Decimal::ZERO,
        price: Decimal::ZERO,
        amount: amt,
        currency: Currency::USD,
        open_date: None,
        open_price: None,
        exchange_rate: Some(r),
        amount_rsd: Some(amt * r),
    }
}

fn seed_rates(data_dir: &std::path::Path, rates: &[(&str, &str, &str)]) {
    let mut map = IndexMap::new();
    for (date, currency, rate) in rates {
        map.insert(format!("{date}_{currency}"), (*rate).to_string());
    }
    let json = serde_json::to_string_pretty(&map).unwrap();
    std::fs::write(data_dir.join("rates.json"), json).unwrap();
}

fn seed_transactions(storage: &Storage, txns: &[Transaction]) {
    let json = serde_json::to_string_pretty(txns).unwrap();
    std::fs::write(storage.data_dir().join("transactions.json"), json).unwrap();
}

// ── Moved from test_cli.rs ──────────────────────────────────

#[test]
fn pipeline_list_to_submit() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "decl-a");
    make_draft(&storage, "decl-b");

    let list_output = cmd()
        .args(["list", "--status", "draft", "-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .output()
        .expect("list failed");
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(stdout.contains("decl-a"));
    assert!(stdout.contains("decl-b"));

    cmd()
        .arg("submit")
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .write_stdin(stdout)
        .assert()
        .success();

    let decl_a = storage.get_declaration("decl-a").unwrap();
    let decl_b = storage.get_declaration("decl-b").unwrap();
    assert_eq!(decl_a.status, DeclarationStatus::Pending);
    assert_eq!(decl_b.status, DeclarationStatus::Pending);
}

// ── sync ────────────────────────────────────────────────────

#[test]
fn sync_missing_ibkr_config() {
    let (tmp, _data_dir) = setup_env();

    cmd()
        .args(["sync"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Configuration errors"));
}

#[test]
fn sync_with_invalid_credentials() {
    let (tmp, data_dir) = setup_env();
    let config = serde_json::json!({
        "data_dir": data_dir.to_str().unwrap(),
        "ibkr_token": "fake-token",
        "ibkr_query_id": "fake-query-id",
        "personal_id": "1234567890123",
        "full_name": "Test User",
        "address": "Test St 1",
        "city_code": "12345",
        "phone": "0601234567",
        "email": "test@example.com",
    });
    std::fs::write(tmp.path().join("config.json"), config.to_string()).unwrap();

    cmd()
        .args(["sync"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn sync_output_dir_override() {
    let (tmp, data_dir) = setup_env();
    let config = serde_json::json!({
        "data_dir": data_dir.to_str().unwrap(),
        "ibkr_token": "fake-token",
        "ibkr_query_id": "fake-query-id",
        "personal_id": "1234567890123",
        "full_name": "Test User",
        "address": "Test St 1",
        "city_code": "12345",
        "phone": "0601234567",
        "email": "test@example.com",
    });
    std::fs::write(tmp.path().join("config.json"), config.to_string()).unwrap();

    let custom_output = tmp.path().join("custom-output");

    cmd()
        .args(["sync", "-o", custom_output.to_str().unwrap()])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();
}

// ── fetch ───────────────────────────────────────────────────

#[test]
fn fetch_missing_ibkr_config() {
    let (tmp, _data_dir) = setup_env();

    cmd()
        .args(["fetch"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Missing IBKR configuration"));
}

#[test]
fn fetch_with_invalid_credentials() {
    let (tmp, data_dir) = setup_env();
    let config = serde_json::json!({
        "data_dir": data_dir.to_str().unwrap(),
        "ibkr_token": "fake-token",
        "ibkr_query_id": "fake-query-id",
    });
    std::fs::write(tmp.path().join("config.json"), config.to_string()).unwrap();

    cmd()
        .args(["fetch"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Fetching"))
        .stderr(predicate::str::is_empty().not());
}

// ── list ────────────────────────────────────────────────────

#[test]
fn list_shows_declarations() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_declaration(
        &storage,
        "d-draft",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
        None,
    );
    make_declaration(
        &storage,
        "d-submitted",
        DeclarationType::Ppo,
        DeclarationStatus::Submitted,
        None,
    );
    make_declaration(
        &storage,
        "d-final",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Finalized,
        None,
    );

    let out = cmd()
        .arg("list")
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("d-draft"), "should show draft");
    assert!(stdout.contains("d-submitted"), "should show submitted");
    assert!(
        !stdout.contains("d-final"),
        "should hide finalized by default"
    );
}

#[test]
fn list_all_includes_finalized() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_declaration(
        &storage,
        "d-draft",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
        None,
    );
    make_declaration(
        &storage,
        "d-final",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Finalized,
        None,
    );

    let out = cmd()
        .args(["list", "--all"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("d-draft"));
    assert!(stdout.contains("d-final"));
}

#[test]
fn list_status_filter() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_declaration(
        &storage,
        "d-draft",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
        None,
    );
    make_declaration(
        &storage,
        "d-sub",
        DeclarationType::Ppo,
        DeclarationStatus::Submitted,
        None,
    );

    let out = cmd()
        .args(["list", "--status", "submitted"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("d-sub"), "should show submitted");
    assert!(!stdout.contains("d-draft"), "should not show draft");
}

#[test]
fn list_ids_only_output() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "id-alpha");
    make_draft(&storage, "id-beta");

    let out = cmd()
        .args(["list", "-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines.contains(&"id-alpha"));
    assert!(lines.contains(&"id-beta"));
}

// ── show ────────────────────────────────────────────────────

#[test]
fn show_existing_declaration() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "show-me");

    cmd()
        .args(["show", "show-me"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("show-me")
                .and(predicate::str::contains("draft"))
                .and(predicate::str::contains("PPDG-3R")),
        );
}

// ── submit ──────────────────────────────────────────────────

#[test]
fn submit_draft_to_pending() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "sub-1");

    cmd()
        .args(["submit", "sub-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Submitted").and(predicate::str::contains("pending")));

    let decl = storage.get_declaration("sub-1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Pending);
}

#[test]
fn submit_ppo_zero_tax_finalizes() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_declaration(
        &storage,
        "ppo-zero",
        DeclarationType::Ppo,
        DeclarationStatus::Draft,
        Some("0.00"),
    );

    cmd()
        .args(["submit", "ppo-zero"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Finalized").and(predicate::str::contains("no tax to pay")),
        );

    let decl = storage.get_declaration("ppo-zero").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Finalized);
}

#[test]
fn submit_multiple_positional_args() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "pos-a");
    make_draft(&storage, "pos-b");

    cmd()
        .args(["submit", "pos-a", "pos-b"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("pos-a").and(predicate::str::contains("pos-b")));

    assert_eq!(
        storage.get_declaration("pos-a").unwrap().status,
        DeclarationStatus::Pending,
    );
    assert_eq!(
        storage.get_declaration("pos-b").unwrap().status,
        DeclarationStatus::Pending,
    );
}

// ── pay ─────────────────────────────────────────────────────

#[test]
fn pay_pending_declaration() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "pay-1");

    cmd()
        .args(["submit", "pay-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();
    assert_eq!(
        storage.get_declaration("pay-1").unwrap().status,
        DeclarationStatus::Pending,
    );

    cmd()
        .args(["pay", "pay-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Paid"));

    let decl = storage.get_declaration("pay-1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Finalized);
}

#[test]
fn pay_submitted_with_tax_flag() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_declaration(
        &storage,
        "pay-tax",
        DeclarationType::Ppo,
        DeclarationStatus::Draft,
        Some("500.00"),
    );

    cmd()
        .args(["submit", "pay-tax"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();
    assert_eq!(
        storage.get_declaration("pay-tax").unwrap().status,
        DeclarationStatus::Submitted,
    );

    cmd()
        .args(["pay", "--tax", "1500", "pay-tax"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("1500").and(predicate::str::contains("RSD")));

    let decl = storage.get_declaration("pay-tax").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Finalized);
    assert_eq!(
        decl.metadata
            .get("assessed_tax_due_rsd")
            .unwrap()
            .as_str()
            .unwrap(),
        "1500.00",
    );
    assert_eq!(
        decl.metadata.get("tax_due_rsd").unwrap().as_str().unwrap(),
        "1500.00",
    );
}

// ── revert ──────────────────────────────────────────────────

#[test]
fn revert_finalized_to_draft() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "rev-1");

    cmd()
        .args(["submit", "rev-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();
    cmd()
        .args(["pay", "rev-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();
    assert_eq!(
        storage.get_declaration("rev-1").unwrap().status,
        DeclarationStatus::Finalized,
    );

    cmd()
        .args(["revert", "rev-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Reverted"));

    assert_eq!(
        storage.get_declaration("rev-1").unwrap().status,
        DeclarationStatus::Draft,
    );
}

#[test]
fn revert_draft_to_submitted() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_declaration(
        &storage,
        "rev-sub",
        DeclarationType::Ppo,
        DeclarationStatus::Draft,
        Some("100.00"),
    );

    cmd()
        .args(["revert", "--to", "submitted", "rev-sub"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Submitted"));

    assert_eq!(
        storage.get_declaration("rev-sub").unwrap().status,
        DeclarationStatus::Submitted,
    );
}

// ── assess ──────────────────────────────────────────────────

#[test]
fn assess_pending_declaration() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "asx-1");

    cmd()
        .args(["submit", "asx-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();

    cmd()
        .args(["assess", "asx-1", "--tax-due", "5000"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Assessment saved"));

    let decl = storage.get_declaration("asx-1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Submitted);
    assert_eq!(
        decl.metadata
            .get("assessed_tax_due_rsd")
            .unwrap()
            .as_str()
            .unwrap(),
        "5000.00",
    );
    assert_eq!(
        decl.metadata.get("tax_due_rsd").unwrap().as_str().unwrap(),
        "5000.00",
    );
}

#[test]
fn assess_with_paid_flag() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "asx-2");

    cmd()
        .args(["submit", "asx-2"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();

    cmd()
        .args(["assess", "asx-2", "--tax-due", "5000", "--paid"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("paid"));

    let decl = storage.get_declaration("asx-2").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Finalized);
    assert_eq!(
        decl.metadata
            .get("assessed_tax_due_rsd")
            .unwrap()
            .as_str()
            .unwrap(),
        "5000.00",
    );
}

// ── export / export-flex ────────────────────────────────────

#[test]
fn export_declaration_xml() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "exp-1");

    let export_dir = tmp.path().join("export-out");
    std::fs::create_dir_all(&export_dir).unwrap();

    cmd()
        .args(["export", "exp-1", "-o", export_dir.to_str().unwrap()])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported XML"));

    let xml_file = export_dir.join("declaration-exp-1.xml");
    assert!(xml_file.exists(), "exported XML file should exist on disk");
    let content = std::fs::read_to_string(&xml_file).unwrap();
    assert_eq!(content, "<xml/>");
}

#[test]
fn export_declaration_with_attachments() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "exp-att");

    let attachment = tmp.path().join("receipt.pdf");
    std::fs::write(&attachment, "fake-pdf-content").unwrap();

    let manager = DeclarationManager::new(&storage);
    manager.attach_file("exp-att", &attachment).unwrap();

    let export_dir = tmp.path().join("export-att-out");
    std::fs::create_dir_all(&export_dir).unwrap();

    cmd()
        .args(["export", "exp-att", "-o", export_dir.to_str().unwrap()])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Exported XML").and(predicate::str::contains("attached file")),
        );

    assert!(export_dir.join("declaration-exp-att.xml").exists());
    let att_file = export_dir.join("receipt.pdf");
    assert!(
        att_file.exists(),
        "attachment should be copied to export dir"
    );
    assert_eq!(
        std::fs::read_to_string(att_file).unwrap(),
        "fake-pdf-content"
    );
}

#[test]
fn export_flex_saved_report() {
    let (tmp, data_dir) = setup_env();

    let cfg = ibkr_porez::models::UserConfig {
        data_dir: Some(data_dir.to_str().unwrap().to_string()),
        ..Default::default()
    };
    let storage = Storage::with_config(&cfg);

    let date = NaiveDate::from_ymd_opt(2099, 12, 31).unwrap();
    let xml = "<FlexQueryResponse>test-data</FlexQueryResponse>";
    storage.save_raw_report(xml, date).unwrap();

    let flex_dir = storage.flex_queries_dir().to_path_buf();
    let _cleanup = FlexCleanup(&flex_dir, "20991231");

    let out = cmd()
        .args(["export-flex", "2099-12-31", "-o", "-"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout, xml);
}

struct FlexCleanup<'a>(&'a std::path::Path, &'a str);

impl Drop for FlexCleanup<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join(format!("base-{}.xml.zip", self.1)));
        let _ = std::fs::remove_file(self.0.join(format!("delta-{}.patch.zip", self.1)));
    }
}

// ── attach ──────────────────────────────────────────────────

#[test]
fn attach_file_to_declaration() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "att-1");

    let file_to_attach = tmp.path().join("doc.txt");
    std::fs::write(&file_to_attach, "hello").unwrap();

    cmd()
        .args(["attach", "att-1", file_to_attach.to_str().unwrap()])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Attached"));

    let decl = storage.get_declaration("att-1").unwrap();
    assert!(
        decl.attached_files.contains_key("doc.txt"),
        "attachment entry should exist in declaration",
    );
    let rel_path = &decl.attached_files["doc.txt"];
    let full_path = storage.declarations_dir().join(rel_path);
    assert!(
        full_path.exists(),
        "copied attachment file should exist on disk"
    );
}

#[test]
fn detach_file_from_declaration() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "det-1");

    let file_to_attach = tmp.path().join("remove-me.txt");
    std::fs::write(&file_to_attach, "bye").unwrap();

    let manager = DeclarationManager::new(&storage);
    manager.attach_file("det-1", &file_to_attach).unwrap();

    let decl_before = storage.get_declaration("det-1").unwrap();
    let rel_path = decl_before.attached_files["remove-me.txt"].clone();
    let full_path = storage.declarations_dir().join(&rel_path);
    assert!(full_path.exists(), "precondition: attachment file exists");

    cmd()
        .args(["attach", "det-1", "--delete", "--file-id", "remove-me.txt"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed"));

    let decl_after = storage.get_declaration("det-1").unwrap();
    assert!(
        !decl_after.attached_files.contains_key("remove-me.txt"),
        "attachment entry should be removed",
    );
    assert!(
        !full_path.exists(),
        "attachment file should be deleted from disk"
    );
}

#[test]
fn attach_nonexistent_file_fails() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "att-nf");

    cmd()
        .args(["attach", "att-nf", "/no/such/file.txt"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("file not found"));
}

#[test]
fn attach_delete_without_identifier_fails() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "att-noid");

    cmd()
        .args(["attach", "att-noid", "--delete"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("file identifier required"));
}

// ── export edge cases ──────────────────────────────────────

#[test]
fn export_no_xml_no_attachments() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    let decl = Declaration {
        declaration_id: "exp-empty".into(),
        r#type: DeclarationType::Ppdg3r,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
        created_at: Local::now().naive_local(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: None,
        report_data: None,
        metadata: IndexMap::new(),
        attached_files: IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();

    let export_dir = tmp.path().join("export-empty");
    std::fs::create_dir_all(&export_dir).unwrap();

    cmd()
        .args(["export", "exp-empty", "-o", export_dir.to_str().unwrap()])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No files to export"));
}

// ── report edge cases ──────────────────────────────────────

#[test]
fn report_gains_no_trades_shows_error() {
    let (tmp, data_dir) = setup_env();
    let config = serde_json::json!({
        "data_dir": data_dir.to_str().unwrap(),
        "output_folder": tmp.path().join("output").to_str().unwrap(),
    });
    std::fs::write(tmp.path().join("config.json"), config.to_string()).unwrap();
    std::fs::create_dir_all(tmp.path().join("output")).unwrap();

    cmd()
        .args([
            "report",
            "--type",
            "gains",
            "--half",
            "2025-1",
            "-o",
            tmp.path().join("output").to_str().unwrap(),
        ])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("no taxable sales"));
}

#[test]
fn report_income_force_flag() {
    let (tmp, _data_dir) = setup_report_env();
    let output_dir = tmp.path().join("force-output");

    cmd()
        .args([
            "report",
            "--type",
            "income",
            "--half",
            "2025-1",
            "--force",
            "-o",
            output_dir.to_str().unwrap(),
        ])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("WARNING").and(predicate::str::contains("Report written")),
        );
}

// ── stat ────────────────────────────────────────────────────

fn setup_stat_data() -> (tempfile::TempDir, PathBuf, Storage) {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);

    let buy1 = make_trade(
        "buy-aapl-1",
        NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
        "AAPL",
        "10",
        "150.00",
        "-1500.00",
        None,
        None,
        "108.00",
    );
    let sell1 = make_trade(
        "sell-aapl-1",
        NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
        "AAPL",
        "-10",
        "170.00",
        "1700.00",
        Some(NaiveDate::from_ymd_opt(2025, 1, 10).unwrap()),
        Some("150.00"),
        "108.50",
    );
    let buy2 = make_trade(
        "buy-msft-1",
        NaiveDate::from_ymd_opt(2025, 2, 5).unwrap(),
        "MSFT",
        "5",
        "300.00",
        "-1500.00",
        None,
        None,
        "107.50",
    );
    let sell2 = make_trade(
        "sell-msft-1",
        NaiveDate::from_ymd_opt(2025, 6, 20).unwrap(),
        "MSFT",
        "-5",
        "320.00",
        "1600.00",
        Some(NaiveDate::from_ymd_opt(2025, 2, 5).unwrap()),
        Some("300.00"),
        "109.00",
    );
    let div1 = make_dividend(
        "div-aapl-1",
        NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
        "AAPL",
        "25.00",
        "108.00",
    );

    seed_transactions(&storage, &[buy1, sell1, buy2, sell2, div1]);

    seed_rates(
        &data_dir,
        &[
            ("2025-01-10", "USD", "108.00"),
            ("2025-03-15", "USD", "108.50"),
            ("2025-02-05", "USD", "107.50"),
            ("2025-06-20", "USD", "109.00"),
            ("2025-03-10", "USD", "108.00"),
        ],
    );

    (tmp, data_dir, storage)
}

#[test]
fn stat_aggregated_view() {
    let (tmp, _data_dir, _storage) = setup_stat_data();
    cmd()
        .arg("stat")
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("AAPL")
                .and(predicate::str::contains("MSFT"))
                .and(predicate::str::contains("2025-03"))
                .and(predicate::str::contains("2025-06")),
        );
}

#[test]
fn stat_year_filter() {
    let (tmp, _data_dir, _storage) = setup_stat_data();
    cmd()
        .args(["stat", "--year", "2025"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("AAPL").and(predicate::str::contains("MSFT")));
}

#[test]
fn stat_ticker_detailed() {
    let (tmp, _data_dir, _storage) = setup_stat_data();
    cmd()
        .args(["stat", "--ticker", "AAPL"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Detailed Report")
                .and(predicate::str::contains("2025-03-15"))
                .and(predicate::str::contains("Total P/L")),
        );
}

#[test]
fn stat_month_filter() {
    let (tmp, _data_dir, _storage) = setup_stat_data();
    cmd()
        .args(["stat", "--month", "2025-03"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("AAPL"));
}

#[test]
fn stat_empty_db() {
    let (tmp, _data_dir) = setup_env();
    cmd()
        .arg("stat")
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No transactions found"));
}

#[test]
fn stat_no_matching_ticker() {
    let (tmp, _data_dir, _storage) = setup_stat_data();
    cmd()
        .args(["stat", "--ticker", "GOOG"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No sales found"));
}

// ── report ──────────────────────────────────────────────────

fn setup_report_env() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();

    let config = serde_json::json!({
        "data_dir": data_dir.to_str().unwrap(),
        "output_folder": tmp.path().join("output").to_str().unwrap(),
        "personal_id": "1234567890123",
        "full_name": "Test User",
        "address": "Test Address 1",
        "city_code": "11000",
        "phone": "+381111111111",
        "email": "test@test.com",
    });
    std::fs::write(tmp.path().join("config.json"), config.to_string()).unwrap();
    std::fs::create_dir_all(tmp.path().join("output")).unwrap();

    let storage = Storage::with_dir(&data_dir);

    let buy = make_trade(
        "buy-aapl-r",
        NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
        "AAPL",
        "10",
        "150.00",
        "-1500.00",
        None,
        None,
        "108.00",
    );
    let sell = make_trade(
        "sell-aapl-r",
        NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
        "AAPL",
        "-10",
        "170.00",
        "1700.00",
        Some(NaiveDate::from_ymd_opt(2025, 1, 10).unwrap()),
        Some("150.00"),
        "108.50",
    );
    let div = make_dividend(
        "div-aapl-r",
        NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
        "AAPL",
        "25.00",
        "108.00",
    );
    let wht = Transaction {
        transaction_id: "wht-aapl-r".into(),
        date: NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
        r#type: TransactionType::WithholdingTax,
        symbol: "AAPL".into(),
        description: "AAPL(US0378331005) - Tax".into(),
        quantity: Decimal::ZERO,
        price: Decimal::ZERO,
        amount: Decimal::from_str("-3.75").unwrap(),
        currency: Currency::USD,
        open_date: None,
        open_price: None,
        exchange_rate: Some(Decimal::from_str("108.00").unwrap()),
        amount_rsd: Some(Decimal::from_str("-405.00").unwrap()),
    };

    seed_transactions(&storage, &[buy, sell, div, wht]);

    seed_rates(
        &data_dir,
        &[
            ("2025-01-10", "USD", "108.00"),
            ("2025-03-15", "USD", "108.50"),
            ("2025-03-10", "USD", "108.00"),
        ],
    );

    (tmp, data_dir)
}

#[test]
fn report_gains() {
    let (tmp, _data_dir) = setup_report_env();
    let output_dir = tmp.path().join("output");

    cmd()
        .args([
            "report",
            "--type",
            "gains",
            "--half",
            "2025-1",
            "-o",
            output_dir.to_str().unwrap(),
        ])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("PPDG-3R")
                .and(predicate::str::contains("AAPL"))
                .and(predicate::str::contains("Report written")),
        );

    let report_file = output_dir.join("ppdg3r-2025-H1.xml");
    assert!(report_file.exists(), "gains report XML should be created");
    let content = std::fs::read_to_string(&report_file).unwrap();
    assert!(content.contains("xml"), "report should contain XML content");
}

#[test]
fn report_gains_default_type() {
    let (tmp, _data_dir) = setup_report_env();
    let output_dir = tmp.path().join("output");

    cmd()
        .args([
            "report",
            "--half",
            "2025-1",
            "-o",
            output_dir.to_str().unwrap(),
        ])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("PPDG-3R"));
}

#[test]
fn report_income() {
    let (tmp, _data_dir) = setup_report_env();
    let output_dir = tmp.path().join("output");

    cmd()
        .args([
            "report",
            "--type",
            "income",
            "--half",
            "2025-1",
            "-o",
            output_dir.to_str().unwrap(),
        ])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Report written").and(predicate::str::contains("ppopo")));
}

#[test]
fn report_income_no_income() {
    let (tmp, data_dir) = setup_env();
    let config = serde_json::json!({
        "data_dir": data_dir.to_str().unwrap(),
        "output_folder": tmp.path().join("output").to_str().unwrap(),
    });
    std::fs::write(tmp.path().join("config.json"), config.to_string()).unwrap();
    std::fs::create_dir_all(tmp.path().join("output")).unwrap();

    let storage = Storage::with_dir(&data_dir);
    let buy = make_trade(
        "buy-only",
        NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
        "AAPL",
        "10",
        "150.00",
        "-1500.00",
        None,
        None,
        "108.00",
    );
    seed_transactions(&storage, &[buy]);

    cmd()
        .args([
            "report",
            "--type",
            "income",
            "--half",
            "2025-1",
            "-o",
            tmp.path().join("output").to_str().unwrap(),
        ])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No income found"));
}

// ── show (enriched) ─────────────────────────────────────────

fn make_enriched_declaration(
    storage: &Storage,
    id: &str,
    decl_type: DeclarationType,
    report_data: Option<Vec<serde_json::Value>>,
    metadata: IndexMap<String, serde_json::Value>,
    attached_files: IndexMap<String, String>,
) {
    let decl = Declaration {
        declaration_id: id.to_string(),
        r#type: decl_type,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
        created_at: Local::now().naive_local(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: Some("<xml/>".into()),
        report_data,
        metadata,
        attached_files,
    };
    storage.save_declaration(&decl).unwrap();
}

#[test]
fn show_gains_with_report_data() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);

    let entry = serde_json::json!({
        "ticker": "AAPL",
        "quantity": "10",
        "sale_date": "2025-03-15",
        "sale_price": "170.00",
        "sale_exchange_rate": "108.50",
        "sale_value_rsd": "184450.00",
        "purchase_date": "2025-01-10",
        "purchase_price": "150.00",
        "purchase_exchange_rate": "108.00",
        "purchase_value_rsd": "162000.00",
        "capital_gain_rsd": "22450.00",
        "is_tax_exempt": false
    });

    make_enriched_declaration(
        &storage,
        "show-gains",
        DeclarationType::Ppdg3r,
        Some(vec![entry]),
        IndexMap::new(),
        IndexMap::new(),
    );

    cmd()
        .args(["show", "show-gains"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("show-gains")
                .and(predicate::str::contains("PPDG-3R"))
                .and(predicate::str::contains("Part 4"))
                .and(predicate::str::contains("AAPL")),
        );
}

#[test]
fn show_income_with_report_data() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);

    let entry = serde_json::json!({
        "date": "2025-03-10",
        "symbol_or_currency": "AAPL",
        "sifra_vrste_prihoda": "111402000",
        "bruto_prihod": "2700.00",
        "osnovica_za_porez": "2700.00",
        "obracunati_porez": "405.00",
        "porez_placen_drugoj_drzavi": "405.00",
        "porez_za_uplatu": "0.00"
    });

    make_enriched_declaration(
        &storage,
        "show-income",
        DeclarationType::Ppo,
        Some(vec![entry]),
        IndexMap::new(),
        IndexMap::new(),
    );

    cmd()
        .args(["show", "show-income"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("show-income")
                .and(predicate::str::contains("PP OPO"))
                .and(predicate::str::contains("111402000")),
        );
}

#[test]
fn show_with_metadata() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);

    let mut metadata = IndexMap::new();
    metadata.insert(
        "tax_due_rsd".into(),
        serde_json::Value::String("5000.00".into()),
    );
    metadata.insert("entry_count".into(), serde_json::Value::from(3));

    make_enriched_declaration(
        &storage,
        "show-meta",
        DeclarationType::Ppdg3r,
        None,
        metadata,
        IndexMap::new(),
    );

    cmd()
        .args(["show", "show-meta"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("tax_due_rsd")
                .and(predicate::str::contains("5000.00"))
                .and(predicate::str::contains("entry_count")),
        );
}

#[test]
fn show_with_attachments() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "show-att");

    let file = tmp.path().join("receipt.pdf");
    std::fs::write(&file, "fake-pdf").unwrap();
    let manager = DeclarationManager::new(&storage);
    manager.attach_file("show-att", &file).unwrap();

    cmd()
        .args(["show", "show-att"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Attached files").and(predicate::str::contains("receipt.pdf")),
        );
}

// ── export-flex extra paths ─────────────────────────────────

#[test]
fn export_flex_to_file() {
    let (tmp, data_dir) = setup_env();
    let cfg = ibkr_porez::models::UserConfig {
        data_dir: Some(data_dir.to_str().unwrap().to_string()),
        ..Default::default()
    };
    let storage = Storage::with_config(&cfg);
    let date = NaiveDate::from_ymd_opt(2098, 6, 15).unwrap();
    let xml = "<FlexQueryResponse>export-test</FlexQueryResponse>";
    storage.save_raw_report(xml, date).unwrap();

    let flex_dir = storage.flex_queries_dir().to_path_buf();
    let _cleanup = FlexCleanup(&flex_dir, "20980615");

    let out_file = tmp.path().join("exported.xml");
    cmd()
        .args([
            "export-flex",
            "2098-06-15",
            "-o",
            out_file.to_str().unwrap(),
        ])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported flex query saved"));

    let content = std::fs::read_to_string(&out_file).unwrap();
    assert!(content.contains("export-test"));
}

#[test]
fn export_flex_to_stdout() {
    let (tmp, data_dir) = setup_env();
    let cfg = ibkr_porez::models::UserConfig {
        data_dir: Some(data_dir.to_str().unwrap().to_string()),
        ..Default::default()
    };
    let storage = Storage::with_config(&cfg);
    let date = NaiveDate::from_ymd_opt(2098, 7, 20).unwrap();
    let xml = "<FlexQueryResponse>stdout-test</FlexQueryResponse>";
    storage.save_raw_report(xml, date).unwrap();

    let flex_dir = storage.flex_queries_dir().to_path_buf();
    let _cleanup = FlexCleanup(&flex_dir, "20980720");

    cmd()
        .args(["export-flex", "2098-07-20", "-o", "-"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout-test"));
}

// ── Full lifecycle ──────────────────────────────────────────

#[test]
fn full_lifecycle_submit_pay_revert() {
    let (tmp, data_dir) = setup_env();
    let storage = Storage::with_dir(&data_dir);
    make_draft(&storage, "life-1");

    cmd()
        .args(["submit", "life-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();
    assert_eq!(
        storage.get_declaration("life-1").unwrap().status,
        DeclarationStatus::Pending,
    );

    cmd()
        .args(["pay", "life-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();
    assert_eq!(
        storage.get_declaration("life-1").unwrap().status,
        DeclarationStatus::Finalized,
    );

    cmd()
        .args(["revert", "life-1"])
        .env("IBKR_POREZ_CONFIG_DIR", tmp.path())
        .assert()
        .success();
    assert_eq!(
        storage.get_declaration("life-1").unwrap().status,
        DeclarationStatus::Draft,
    );
}
