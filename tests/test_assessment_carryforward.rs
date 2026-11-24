use chrono::{Datelike, Local, NaiveDate};
use ibkr_porez::declaration_manager::{AssessmentInput, DeclarationManager};
use ibkr_porez::delete::{execute_deletion, plan_deletion};
use ibkr_porez::models::{
    CarryforwardVintage, Declaration, DeclarationStatus, DeclarationType, UserConfig,
};
use ibkr_porez::prior_loss_use;
use ibkr_porez::storage::Storage;
use indexmap::IndexMap;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

struct Env {
    _tmp: tempfile::TempDir,
    storage: Storage,
    cfg: UserConfig,
}

fn setup() -> Env {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();
    let cfg = UserConfig {
        output_folder: Some(output_dir.display().to_string()),
        ..UserConfig::default()
    };
    Env {
        _tmp: tmp,
        storage,
        cfg,
    }
}

fn seed_vintage(
    storage: &Storage,
    origin: &str,
    period: (NaiveDate, NaiveDate),
    recognized: Decimal,
    remaining: Decimal,
) {
    storage
        .upsert_carryforward_vintage(CarryforwardVintage {
            id: format!("CF-{origin}"),
            origin_declaration_id: origin.into(),
            assessment_reference: Some(format!("REF-{origin}")),
            origin_period_start: period.0,
            origin_period_end: period.1,
            recognized_loss_rsd: recognized,
            remaining_loss_rsd: remaining,
            created_at: Local::now().naive_local(),
            expiration_tax_year: period.1.year() + 5,
            notes: None,
        })
        .unwrap();
}

fn seed_gains_decl(
    storage: &Storage,
    id: &str,
    period: (NaiveDate, NaiveDate),
    status: DeclarationStatus,
    metadata: IndexMap<String, serde_json::Value>,
) -> Declaration {
    let now = Local::now().naive_local();
    let mut attached_files = IndexMap::new();
    attached_files.insert(
        "decision.pdf".to_string(),
        format!("{id}/attachments/decision.pdf"),
    );
    let decl = Declaration {
        declaration_id: id.into(),
        r#type: DeclarationType::Ppdg3r,
        status,
        period_start: period.0,
        period_end: period.1,
        created_at: now,
        submitted_at: Some(now),
        paid_at: None,
        file_path: None,
        xml_content: Some("<xml/>".into()),
        report_data: None,
        metadata,
        attached_files,
    };
    storage.save_declaration(&decl).unwrap();
    decl
}

fn sources_meta(sources: &[(&str, &str)]) -> IndexMap<String, serde_json::Value> {
    let total: Decimal = sources
        .iter()
        .map(|(_, a)| a.parse::<Decimal>().unwrap())
        .sum();
    let mut m = IndexMap::new();
    m.insert("carryforward_used_rsd".into(), format!("{total:.2}").into());
    m.insert(
        "carryforward_sources".into(),
        serde_json::Value::Array(
            sources
                .iter()
                .map(|(v, a)| serde_json::json!({ "vintage_id": v, "amount_used": a }))
                .collect(),
        ),
    );
    m
}

fn remaining(storage: &Storage, id: &str) -> Decimal {
    storage
        .find_carryforward_vintage(id)
        .unwrap()
        .remaining_loss_rsd
}

fn total_remaining(storage: &Storage) -> Decimal {
    storage
        .get_carryforward_vintages()
        .iter()
        .map(|v| v.remaining_loss_rsd)
        .sum()
}

fn h2_2025() -> (NaiveDate, NaiveDate) {
    (d(2025, 7, 1), d(2025, 12, 31))
}
fn h1_2026() -> (NaiveDate, NaiveDate) {
    (d(2026, 1, 1), d(2026, 6, 30))
}

/// Declaration 11 as left by the first assessment entry: finalized, its own
/// 3,945.67 RSD loss recorded, but the provisional 9,488.47 RSD deduction
/// from CF-1 still reflected in the ledger.
fn seed_declaration_11(storage: &Storage) -> Declaration {
    seed_vintage(storage, "1", h2_2025(), dec!(44471.30), dec!(34982.83));
    seed_vintage(storage, "11", h1_2026(), dec!(3945.67), dec!(3945.67));
    let mut meta = sources_meta(&[("CF-1", "9488.47")]);
    meta.insert("total_gain_rsd".into(), "9488.47".into());
    meta.insert("estimated_tax_rsd".into(), "0.00".into());
    meta.insert("recognized_capital_loss_rsd".into(), "3945.67".into());
    meta.insert("assessed_tax_due_rsd".into(), "0.00".into());
    meta.insert("tax_due_rsd".into(), "0.00".into());
    meta.insert("assessment_reference".into(), "PURS-11/2026".into());
    meta.insert("assessment_date".into(), "2026-10-15".into());
    seed_gains_decl(storage, "11", h1_2026(), DeclarationStatus::Finalized, meta)
}

fn purs_decision_11() -> AssessmentInput {
    AssessmentInput {
        assessed_tax_rsd: Some(dec!(0)),
        recognized_capital_gain_rsd: Some(dec!(0)),
        recognized_capital_loss_rsd: Some(dec!(3945.67)),
        ..Default::default()
    }
}

fn assert_declaration_11_matches_purs(storage: &Storage) {
    assert_eq!(remaining(storage, "CF-1"), dec!(44471.30));
    assert_eq!(remaining(storage, "CF-11"), dec!(3945.67));
    assert_eq!(total_remaining(storage), dec!(48416.97));

    let decl = storage.get_declaration("11").unwrap();
    let mgr = DeclarationManager::new(storage);
    assert_eq!(mgr.tax_due_rsd(&decl), dec!(0));
    assert_eq!(decl.status, DeclarationStatus::Finalized);
    assert_eq!(
        prior_loss_use::assessed_deductions(&decl).unwrap(),
        Some(Vec::new())
    );
    let original = prior_loss_use::original_deductions(&decl).unwrap();
    assert_eq!(original.len(), 1);
    assert_eq!(original[0].vintage_id, "CF-1");
    assert_eq!(original[0].amount_used, dec!(9488.47));
    assert_eq!(decl.metadata["assessment_reference"], "PURS-11/2026");
    assert_eq!(decl.metadata["assessment_date"], "2026-10-15");
    assert!(decl.attached_files.contains_key("decision.pdf"));
}

#[test]
fn declaration_11_assessment_restores_provisional_deduction_and_is_repeatable() {
    let env = setup();
    seed_declaration_11(&env.storage);
    assert_eq!(total_remaining(&env.storage), dec!(38928.50));

    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment("11", &purs_decision_11()).unwrap();
    assert_declaration_11_matches_purs(&env.storage);

    mgr.record_assessment("11", &purs_decision_11()).unwrap();
    assert_declaration_11_matches_purs(&env.storage);
}

#[test]
fn provisional_deductions_from_two_decisions_return_to_each_origin() {
    let env = setup();
    seed_vintage(
        &env.storage,
        "1",
        (d(2024, 1, 1), d(2024, 6, 30)),
        dec!(1000),
        dec!(600),
    );
    seed_vintage(&env.storage, "2", h2_2025(), dec!(500), dec!(250));
    seed_vintage(&env.storage, "7", h2_2025(), dec!(800), dec!(800));
    seed_gains_decl(
        &env.storage,
        "3",
        h1_2026(),
        DeclarationStatus::Pending,
        sources_meta(&[("CF-1", "400.00"), ("CF-2", "250.00")]),
    );
    seed_gains_decl(
        &env.storage,
        "4",
        h1_2026(),
        DeclarationStatus::Finalized,
        sources_meta(&[("CF-7", "0.00")]),
    );

    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment(
        "3",
        &AssessmentInput {
            recognized_capital_loss_rsd: Some(dec!(120)),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(remaining(&env.storage, "CF-1"), dec!(1000));
    assert_eq!(remaining(&env.storage, "CF-2"), dec!(500));
    assert_eq!(remaining(&env.storage, "CF-7"), dec!(800));
    assert_eq!(remaining(&env.storage, "CF-3"), dec!(120));
    assert_eq!(total_remaining(&env.storage), dec!(2420));
    assert!(
        prior_loss_use::assessed_deductions(&env.storage.get_declaration("4").unwrap())
            .unwrap()
            .is_none()
    );
}

#[test]
fn tax_only_zero_assessment_keeps_provisional_deductions() {
    let env = setup();
    seed_vintage(&env.storage, "1", h2_2025(), dec!(44471.30), dec!(34982.83));
    seed_gains_decl(
        &env.storage,
        "11",
        h1_2026(),
        DeclarationStatus::Pending,
        sources_meta(&[("CF-1", "9488.47")]),
    );

    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment(
        "11",
        &AssessmentInput {
            assessed_tax_rsd: Some(dec!(0)),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(remaining(&env.storage, "CF-1"), dec!(34982.83));
    let decl = env.storage.get_declaration("11").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Finalized);
    assert_eq!(prior_loss_use::assessed_deductions(&decl).unwrap(), None);
    assert!(
        env.storage
            .load_carryforward_ledger()
            .reflected_deductions
            .is_empty()
    );
}

/// The ledger was saved but the declaration write was lost: the retry must
/// not return the deduction a second time.
#[test]
fn retry_after_lost_declaration_write_applies_difference_once() {
    let env = setup();
    let before = seed_declaration_11(&env.storage);

    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment("11", &purs_decision_11()).unwrap();
    env.storage.save_declaration(&before).unwrap();
    assert_eq!(remaining(&env.storage, "CF-1"), dec!(44471.30));

    mgr.record_assessment("11", &purs_decision_11()).unwrap();
    assert_declaration_11_matches_purs(&env.storage);
}

#[test]
fn delete_after_reconciliation_reverses_only_reflected_deductions() {
    let env = setup();
    seed_declaration_11(&env.storage);
    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment("11", &purs_decision_11()).unwrap();

    let plan = plan_deletion(&env.storage, "11").unwrap();
    execute_deletion(&env.storage, &env.cfg, &plan).unwrap();

    assert_eq!(remaining(&env.storage, "CF-1"), dec!(44471.30));
    assert!(env.storage.find_carryforward_vintage("CF-11").is_none());
    assert_eq!(total_remaining(&env.storage), dec!(44471.30));
    assert!(
        env.storage
            .load_carryforward_ledger()
            .reflected_deductions
            .is_empty()
    );
}

/// Deletion wrote the ledger but the declaration record survived: retrying
/// the deletion must not return the deduction a second time.
#[test]
fn retry_after_interrupted_deletion_restores_once() {
    let env = setup();
    seed_vintage(&env.storage, "1", h2_2025(), dec!(44471.30), dec!(34982.83));
    let decl = seed_gains_decl(
        &env.storage,
        "11",
        h1_2026(),
        DeclarationStatus::Pending,
        sources_meta(&[("CF-1", "9488.47")]),
    );

    let mut ledger = env.storage.load_carryforward_ledger();
    prior_loss_use::reconcile(&mut ledger, &decl, &[]).unwrap();
    env.storage.save_carryforward_ledger(&ledger).unwrap();
    assert_eq!(remaining(&env.storage, "CF-1"), dec!(44471.30));

    let plan = plan_deletion(&env.storage, "11").unwrap();
    execute_deletion(&env.storage, &env.cfg, &plan).unwrap();
    assert_eq!(remaining(&env.storage, "CF-1"), dec!(44471.30));
    assert!(env.storage.get_declaration("11").is_none());
}

/// A record left by a deleted declaration must not apply to a new declaration
/// that reuses its id.
#[test]
fn stale_record_does_not_apply_to_reused_declaration_id() {
    let env = setup();
    seed_vintage(&env.storage, "1", h2_2025(), dec!(1000), dec!(600));
    let old = seed_gains_decl(
        &env.storage,
        "11",
        h1_2026(),
        DeclarationStatus::Pending,
        sources_meta(&[("CF-1", "400.00")]),
    );
    let mut ledger = env.storage.load_carryforward_ledger();
    prior_loss_use::reconcile(&mut ledger, &old, &[]).unwrap();

    let mut reused = old.clone();
    reused.created_at += chrono::Duration::seconds(1);
    let reflected = prior_loss_use::reflected_deductions(&ledger, &reused).unwrap();
    assert_eq!(reflected.len(), 1);
    assert_eq!(reflected[0].amount_used, dec!(400));
}

#[test]
fn revert_after_reconciliation_reapplies_original_deductions() {
    let env = setup();
    seed_declaration_11(&env.storage);
    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment("11", &purs_decision_11()).unwrap();

    mgr.revert(&["11"]).unwrap();

    assert_eq!(remaining(&env.storage, "CF-1"), dec!(34982.83));
    assert!(env.storage.find_carryforward_vintage("CF-11").is_none());
    let decl = env.storage.get_declaration("11").unwrap();
    assert_eq!(decl.status, DeclarationStatus::Draft);
    assert_eq!(prior_loss_use::assessed_deductions(&decl).unwrap(), None);
}

/// Correcting a loss assessment to a positive gain no longer establishes
/// zero prior-loss use, so the original deductions apply again.
#[test]
fn correcting_to_positive_gain_reapplies_original_deductions() {
    let env = setup();
    seed_declaration_11(&env.storage);
    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment("11", &purs_decision_11()).unwrap();

    mgr.record_assessment(
        "11",
        &AssessmentInput {
            recognized_capital_loss_rsd: Some(dec!(0)),
            recognized_capital_gain_rsd: Some(dec!(9488.47)),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(remaining(&env.storage, "CF-1"), dec!(34982.83));
    assert!(env.storage.find_carryforward_vintage("CF-11").is_none());
    let decl = env.storage.get_declaration("11").unwrap();
    assert_eq!(prior_loss_use::assessed_deductions(&decl).unwrap(), None);
}

#[test]
fn restoring_more_than_recognized_is_refused_without_changes() {
    let env = setup();
    seed_vintage(&env.storage, "1", h2_2025(), dec!(1000), dec!(1000));
    seed_gains_decl(
        &env.storage,
        "11",
        h1_2026(),
        DeclarationStatus::Pending,
        sources_meta(&[("CF-1", "400.00")]),
    );

    let mgr = DeclarationManager::new(&env.storage);
    let err = mgr
        .record_assessment(
            "11",
            &AssessmentInput {
                recognized_capital_loss_rsd: Some(dec!(50)),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("CF-1"), "got: {err}");
    assert_eq!(remaining(&env.storage, "CF-1"), dec!(1000));
    assert!(env.storage.find_carryforward_vintage("CF-11").is_none());
    assert!(
        env.storage
            .get_declaration("11")
            .unwrap()
            .metadata
            .get("recognized_capital_loss_rsd")
            .is_none()
    );
}

/// A later declaration already drew on the declaration's own loss: entering
/// the same decision again must still reconcile instead of being refused.
#[test]
fn reentering_unchanged_loss_after_own_vintage_consumed_still_reconciles() {
    let env = setup();
    seed_declaration_11(&env.storage);
    env.storage
        .apply_carryforward_consumption(&[ibkr_porez::models::CarryforwardSource {
            vintage_id: "CF-11".into(),
            amount_used: dec!(1000),
        }])
        .unwrap();

    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment("11", &purs_decision_11()).unwrap();

    assert_eq!(remaining(&env.storage, "CF-1"), dec!(44471.30));
    assert_eq!(remaining(&env.storage, "CF-11"), dec!(2945.67));

    let err = mgr
        .record_assessment(
            "11",
            &AssessmentInput {
                recognized_capital_loss_rsd: Some(dec!(4000)),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("partially consumed"), "got: {err}");
    assert_eq!(remaining(&env.storage, "CF-11"), dec!(2945.67));
}

/// Revert discards the assessment, so a later tax-only assessment of the
/// resubmitted declaration must not re-establish zero prior-loss use from it.
#[test]
fn tax_only_assessment_after_revert_keeps_provisional_deductions() {
    let env = setup();
    seed_declaration_11(&env.storage);
    let mgr = DeclarationManager::new(&env.storage);
    mgr.record_assessment("11", &purs_decision_11()).unwrap();
    mgr.revert(&["11"]).unwrap();
    mgr.submit(&["11"]).unwrap();

    mgr.record_assessment(
        "11",
        &AssessmentInput {
            assessed_tax_rsd: Some(dec!(1423.27)),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(remaining(&env.storage, "CF-1"), dec!(34982.83));
    let decl = env.storage.get_declaration("11").unwrap();
    assert_eq!(prior_loss_use::assessed_deductions(&decl).unwrap(), None);
}
