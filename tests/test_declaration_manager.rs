use chrono::{Local, NaiveDate, NaiveDateTime};
use ibkr_porez::declaration_manager::DeclarationManager;
use ibkr_porez::models::{Declaration, DeclarationStatus, DeclarationType};
use ibkr_porez::storage::Storage;
use indexmap::IndexMap;
use rust_decimal_macros::dec;

fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

fn make_declaration(
    storage: &Storage,
    id: &str,
    decl_type: DeclarationType,
    status: DeclarationStatus,
) -> Declaration {
    let decl = Declaration {
        declaration_id: id.to_string(),
        r#type: decl_type,
        status,
        period_start: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        created_at: now(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: Some("<xml/>".into()),
        report_data: None,
        metadata: IndexMap::new(),
        attached_files: IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();
    decl
}

#[test]
fn test_submit_ppdg3r_goes_to_pending() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
    );

    let mgr = DeclarationManager::new(&storage);
    mgr.submit(&["1"]).unwrap();

    let decl = storage.get_declaration("1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Pending);
    assert!(decl.submitted_at.is_some());
}

#[test]
fn test_submit_ppopo_with_tax_goes_to_submitted() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    let mut metadata = IndexMap::new();
    metadata.insert("tax_due_rsd".to_string(), serde_json::json!("50.00"));

    let mut decl = Declaration {
        declaration_id: "1".to_string(),
        r#type: DeclarationType::Ppo,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        created_at: now(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: Some("<xml/>".into()),
        report_data: None,
        metadata,
        attached_files: IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();

    let mgr = DeclarationManager::new(&storage);
    mgr.submit(&["1"]).unwrap();

    decl = storage.get_declaration("1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Submitted);
}

#[test]
fn test_submit_ppopo_zero_tax_goes_to_finalized() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    let mut metadata = IndexMap::new();
    metadata.insert("tax_due_rsd".to_string(), serde_json::json!("0.00"));

    let decl = Declaration {
        declaration_id: "1".to_string(),
        r#type: DeclarationType::Ppo,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2023, 7, 15).unwrap(),
        created_at: now(),
        submitted_at: None,
        paid_at: None,
        file_path: None,
        xml_content: Some("<xml/>".into()),
        report_data: None,
        metadata,
        attached_files: IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();

    let mgr = DeclarationManager::new(&storage);
    mgr.submit(&["1"]).unwrap();

    let updated = storage.get_declaration("1").unwrap();
    assert_eq!(updated.status, DeclarationStatus::Finalized);
}

#[test]
fn test_submit_non_draft_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Submitted,
    );

    let mgr = DeclarationManager::new(&storage);
    let result = mgr.submit(&["1"]);
    assert!(result.is_err());
}

#[test]
fn test_pay_sets_finalized() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    mgr.pay(&["1"]).unwrap();

    let decl = storage.get_declaration("1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Finalized);
    assert!(decl.paid_at.is_some());
}

#[test]
fn test_pay_already_finalized_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Finalized,
    );

    let mgr = DeclarationManager::new(&storage);
    let result = mgr.pay(&["1"]);
    assert!(result.is_err());
}

#[test]
fn test_revert_to_draft() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Finalized,
    );

    let mgr = DeclarationManager::new(&storage);
    mgr.revert(&["1"]).unwrap();

    let decl = storage.get_declaration("1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Draft);
    assert!(decl.submitted_at.is_none());
    assert!(decl.paid_at.is_none());
}

#[test]
fn test_set_assessed_tax_on_draft_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
    );

    let mgr = DeclarationManager::new(&storage);
    let result = mgr.set_assessed_tax("1", dec!(100), false);
    assert!(result.is_err());
}

#[test]
fn test_set_assessed_tax_and_mark_paid() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    mgr.set_assessed_tax("1", dec!(500), true).unwrap();

    let decl = storage.get_declaration("1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Finalized);
    assert!(decl.paid_at.is_some());
    assert_eq!(mgr.tax_due_rsd(&decl), dec!(500));
}

#[test]
fn test_tax_due_rsd_defaults_to_one() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let decl = make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
    );

    let mgr = DeclarationManager::new(&storage);
    assert_eq!(mgr.tax_due_rsd(&decl), dec!(1));
}

#[test]
fn test_export_creates_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    let decl = Declaration {
        declaration_id: "1".to_string(),
        r#type: DeclarationType::Ppdg3r,
        status: DeclarationStatus::Draft,
        period_start: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
        created_at: now(),
        submitted_at: None,
        paid_at: None,
        file_path: Some("001-ppdg3r-2023-H1.xml".into()),
        xml_content: Some("<xml>test</xml>".into()),
        report_data: None,
        metadata: IndexMap::new(),
        attached_files: IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();

    let output_dir = tmp.path().join("export");
    let mgr = DeclarationManager::new(&storage);
    let files = mgr.export("1", &output_dir).unwrap();

    assert!(files.xml_path.is_some());
    let content = std::fs::read_to_string(files.xml_path.unwrap()).unwrap();
    assert_eq!(content, "<xml>test</xml>");
}

#[test]
fn test_attach_and_detach_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
    );

    let attachment = tmp.path().join("doc.pdf");
    std::fs::write(&attachment, b"pdf content").unwrap();

    let mgr = DeclarationManager::new(&storage);
    let name = mgr.attach_file("1", &attachment).unwrap();
    assert_eq!(name, "doc.pdf");

    let decl = storage.get_declaration("1").unwrap();
    assert!(decl.attached_files.contains_key("doc.pdf"));

    mgr.detach_file("1", "doc.pdf").unwrap();
    let decl = storage.get_declaration("1").unwrap();
    assert!(!decl.attached_files.contains_key("doc.pdf"));
}
