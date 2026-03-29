use std::path::PathBuf;

use assert_cmd::Command;
use chrono::{Local, NaiveDate};
use ibkr_porez::declaration_manager::DeclarationManager;
use ibkr_porez::models::{Declaration, DeclarationStatus, DeclarationType};
use ibkr_porez::storage::Storage;
use indexmap::IndexMap;
use predicates::prelude::*;

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
