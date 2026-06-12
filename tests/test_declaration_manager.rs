use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use ibkr_porez::declaration_manager::{AssessmentInput, DeclarationManager};
use ibkr_porez::models::{CarryforwardVintage, Declaration, DeclarationStatus, DeclarationType};
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
fn test_record_assessment_on_draft_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        assessed_tax_rsd: Some(dec!(100)),
        ..Default::default()
    };
    let result = mgr.record_assessment("1", &input);
    assert!(result.is_err());
}

#[test]
fn test_record_assessment_and_mark_paid() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        assessed_tax_rsd: Some(dec!(500)),
        mark_paid: true,
        ..Default::default()
    };
    mgr.record_assessment("1", &input).unwrap();

    let decl = storage.get_declaration("1").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Finalized);
    assert!(decl.paid_at.is_some());
    assert_eq!(mgr.tax_due_rsd(&decl), dec!(500));
}

#[test]
fn test_record_assessment_requires_at_least_one_field() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let result = mgr.record_assessment("1", &AssessmentInput::default());
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("at least one of --tax, --gain, or --loss")
    );
}

#[test]
fn test_record_assessment_recognized_gain_loss_only_for_ppdg3r() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppo,
        DeclarationStatus::Submitted,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(100)),
        ..Default::default()
    };
    let result = mgr.record_assessment("1", &input);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("only applies to PPDG-3R")
    );
}

#[test]
fn test_record_assessment_cannot_recognize_both_gain_and_loss() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        recognized_capital_gain_rsd: Some(dec!(100)),
        recognized_capital_loss_rsd: Some(dec!(50)),
        ..Default::default()
    };
    let result = mgr.record_assessment("1", &input);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("cannot recognize both a capital gain and a capital loss")
    );
}

#[test]
fn test_record_assessment_recognized_loss_creates_carryforward_vintage() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let decl = make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(44471.30)),
        assessment_reference: Some("REF-1".into()),
        ..Default::default()
    };
    mgr.record_assessment("1", &input).unwrap();

    let vintage = storage.find_carryforward_vintage("CF-1").unwrap();
    assert_eq!(vintage.recognized_loss_rsd, dec!(44471.30));
    assert_eq!(vintage.remaining_loss_rsd, dec!(44471.30));
    assert_eq!(vintage.origin_declaration_id, "1");
    assert_eq!(vintage.assessment_reference, Some("REF-1".into()));
    assert_eq!(vintage.expiration_tax_year, decl.period_end.year() + 5);

    let saved = storage.get_declaration("1").unwrap();
    assert_eq!(
        saved.metadata.get("recognized_capital_loss_rsd").unwrap(),
        "44471.30"
    );
}

#[test]
fn test_record_assessment_re_run_updates_unconsumed_vintage_in_place() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(1000)),
        ..Default::default()
    };
    mgr.record_assessment("1", &input).unwrap();

    let input2 = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(1500)),
        ..Default::default()
    };
    mgr.record_assessment("1", &input2).unwrap();

    let vintages = storage.get_carryforward_vintages();
    assert_eq!(vintages.len(), 1);
    assert_eq!(vintages[0].recognized_loss_rsd, dec!(1500));
    assert_eq!(vintages[0].remaining_loss_rsd, dec!(1500));
}

#[test]
fn test_record_assessment_fails_after_partial_consumption() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let decl = make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(1000)),
        ..Default::default()
    };
    mgr.record_assessment("1", &input).unwrap();

    // Simulate partial consumption.
    storage
        .upsert_carryforward_vintage(CarryforwardVintage {
            id: "CF-1".into(),
            origin_declaration_id: "1".into(),
            assessment_reference: None,
            origin_period_start: decl.period_start,
            origin_period_end: decl.period_end,
            recognized_loss_rsd: dec!(1000),
            remaining_loss_rsd: dec!(400),
            created_at: now(),
            expiration_tax_year: decl.period_end.year() + 5,
            notes: None,
        })
        .unwrap();

    let input2 = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(1500)),
        ..Default::default()
    };
    let result = mgr.record_assessment("1", &input2);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("already been partially consumed")
    );
}

#[test]
fn test_record_assessment_cannot_add_loss_after_recognized_gain() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let gain_input = AssessmentInput {
        recognized_capital_gain_rsd: Some(dec!(100)),
        ..Default::default()
    };
    mgr.record_assessment("1", &gain_input).unwrap();

    let loss_input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(50)),
        ..Default::default()
    };
    let result = mgr.record_assessment("1", &loss_input);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("cannot recognize both a capital gain and a capital loss")
    );

    // Neither the metadata nor the ledger should reflect the rejected loss.
    let saved = storage.get_declaration("1").unwrap();
    assert!(saved.metadata.get("recognized_capital_loss_rsd").is_none());
    assert!(storage.find_carryforward_vintage("CF-1").is_none());
}

#[test]
fn test_record_assessment_cannot_add_gain_after_recognized_loss() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let loss_input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(1000)),
        ..Default::default()
    };
    mgr.record_assessment("1", &loss_input).unwrap();

    let gain_input = AssessmentInput {
        recognized_capital_gain_rsd: Some(dec!(100)),
        ..Default::default()
    };
    let result = mgr.record_assessment("1", &gain_input);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("cannot recognize both a capital gain and a capital loss")
    );
}

#[test]
fn test_record_assessment_clearing_loss_then_recognizing_gain_succeeds() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let loss_input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(1000)),
        ..Default::default()
    };
    mgr.record_assessment("1", &loss_input).unwrap();

    // Correct the assessment in one go: clear the recognized loss and
    // recognize a gain instead.
    let correction = AssessmentInput {
        recognized_capital_gain_rsd: Some(dec!(100)),
        recognized_capital_loss_rsd: Some(dec!(0)),
        ..Default::default()
    };
    mgr.record_assessment("1", &correction).unwrap();

    assert!(storage.find_carryforward_vintage("CF-1").is_none());
    let saved = storage.get_declaration("1").unwrap();
    assert_eq!(
        saved.metadata.get("recognized_capital_gain_rsd").unwrap(),
        "100.00"
    );
}

#[test]
fn test_revert_removes_unconsumed_carryforward_vintage() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(1000)),
        ..Default::default()
    };
    mgr.record_assessment("1", &input).unwrap();
    assert!(storage.find_carryforward_vintage("CF-1").is_some());

    mgr.revert(&["1"]).unwrap();

    assert!(storage.find_carryforward_vintage("CF-1").is_none());
    let saved = storage.get_declaration("1").unwrap();
    assert_eq!(saved.status, DeclarationStatus::Draft);
}

#[test]
fn test_revert_fails_if_carryforward_vintage_partially_consumed() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let decl = make_declaration(
        &storage,
        "1",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Pending,
    );

    let mgr = DeclarationManager::new(&storage);
    let input = AssessmentInput {
        recognized_capital_loss_rsd: Some(dec!(1000)),
        ..Default::default()
    };
    mgr.record_assessment("1", &input).unwrap();

    // Simulate partial consumption.
    storage
        .upsert_carryforward_vintage(CarryforwardVintage {
            id: "CF-1".into(),
            origin_declaration_id: "1".into(),
            assessment_reference: None,
            origin_period_start: decl.period_start,
            origin_period_end: decl.period_end,
            recognized_loss_rsd: dec!(1000),
            remaining_loss_rsd: dec!(400),
            created_at: now(),
            expiration_tax_year: decl.period_end.year() + 5,
            notes: None,
        })
        .unwrap();

    let result = mgr.revert(&["1"]);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("already been partially consumed")
    );

    // Declaration status must be unchanged since the revert was rejected.
    let saved = storage.get_declaration("1").unwrap();
    assert_eq!(saved.status, DeclarationStatus::Pending);
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

// ---- apply_each tests ----

#[test]
fn apply_each_all_succeed() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    for id in ["a", "b", "c"] {
        make_declaration(
            &storage,
            id,
            DeclarationType::Ppdg3r,
            DeclarationStatus::Draft,
        );
    }

    let mgr = DeclarationManager::new(&storage);
    let result = mgr.apply_each(&["a", "b", "c"], |m, id| m.submit(&[id]));

    assert_eq!(result.ok_count, 3);
    assert!(!result.has_errors());
    assert!(result.errors.is_empty());
}

#[test]
fn apply_each_mixed_results() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    make_declaration(
        &storage,
        "ok",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Draft,
    );
    make_declaration(
        &storage,
        "bad",
        DeclarationType::Ppdg3r,
        DeclarationStatus::Submitted,
    );

    let mgr = DeclarationManager::new(&storage);
    let result = mgr.apply_each(&["ok", "bad"], |m, id| m.submit(&[id]));

    assert_eq!(result.ok_count, 1);
    assert!(result.has_errors());
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].0, "bad");
    assert!(result.errors[0].1.contains("not in Draft"));
}

#[test]
fn apply_each_all_fail() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    let mgr = DeclarationManager::new(&storage);
    let result = mgr.apply_each(&["x", "y"], |m, id| m.submit(&[id]));

    assert_eq!(result.ok_count, 0);
    assert!(result.has_errors());
    assert_eq!(result.errors.len(), 2);
}

#[test]
fn apply_each_error_summary_format() {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());

    let mgr = DeclarationManager::new(&storage);
    let result = mgr.apply_each(&["x", "y"], |m, id| m.submit(&[id]));

    let summary = result.error_summary();
    assert!(summary.contains("x:"));
    assert!(summary.contains("y:"));
}
