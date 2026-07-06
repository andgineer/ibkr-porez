use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate};
use ibkr_porez::delete::{execute_deletion, plan_deletion};
use ibkr_porez::models::{
    CarryforwardVintage, Declaration, DeclarationStatus, DeclarationType, UserConfig,
};
use ibkr_porez::storage::Storage;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ── Fixtures ────────────────────────────────────────────────

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

struct Env {
    _tmp: tempfile::TempDir,
    storage: Storage,
    output_dir: PathBuf,
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
        output_dir,
        cfg,
    }
}

fn seed_vintage(storage: &Storage, id: &str, recognized: Decimal, remaining: Decimal) {
    storage
        .upsert_carryforward_vintage(CarryforwardVintage {
            id: id.into(),
            origin_declaration_id: "origin".into(),
            assessment_reference: None,
            origin_period_start: d(2023, 1, 1),
            origin_period_end: d(2023, 6, 30),
            recognized_loss_rsd: recognized,
            remaining_loss_rsd: remaining,
            created_at: Local::now().naive_local(),
            expiration_tax_year: 2028,
            notes: None,
        })
        .unwrap();
}

fn seed_decl(
    storage: &Storage,
    id: &str,
    dtype: DeclarationType,
    period: (NaiveDate, NaiveDate),
    metadata: indexmap::IndexMap<String, serde_json::Value>,
    file_path: Option<String>,
) {
    let decl = Declaration {
        declaration_id: id.into(),
        r#type: dtype,
        status: DeclarationStatus::Draft,
        period_start: period.0,
        period_end: period.1,
        created_at: Local::now().naive_local(),
        submitted_at: None,
        paid_at: None,
        file_path,
        xml_content: None,
        report_data: None,
        metadata,
        attached_files: indexmap::IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();
}

/// Metadata as written by a gains declaration that consumed `amount` from `vintage`.
fn consumed_meta(vintage: &str, amount: &str) -> indexmap::IndexMap<String, serde_json::Value> {
    let mut m = indexmap::IndexMap::new();
    m.insert(
        "carryforward_sources".into(),
        serde_json::json!([{ "vintage_id": vintage, "amount_used": amount }]),
    );
    m
}

fn h1_2024() -> (NaiveDate, NaiveDate) {
    (d(2024, 1, 1), d(2024, 6, 30))
}
fn h2_2024() -> (NaiveDate, NaiveDate) {
    (d(2024, 7, 1), d(2024, 12, 31))
}

// ── Tests ───────────────────────────────────────────────────

/// Deleting a gains declaration returns the carryforward it consumed to its vintage.
#[test]
fn delete_reverses_consumption() {
    let env = setup();
    // Vintage seeded as already fully consumed by the declaration below.
    seed_vintage(&env.storage, "CF-origin", dec!(50000), Decimal::ZERO);
    seed_decl(
        &env.storage,
        "1",
        DeclarationType::Ppdg3r,
        h1_2024(),
        consumed_meta("CF-origin", "50000.00"),
        None,
    );

    let plan = plan_deletion(&env.storage, "1").unwrap();
    assert!(!plan.deletes_vintage);
    execute_deletion(&env.storage, &env.cfg, &plan).unwrap();

    assert!(env.storage.get_declaration("1").is_none());
    assert_eq!(
        env.storage
            .find_carryforward_vintage("CF-origin")
            .unwrap()
            .remaining_loss_rsd,
        dec!(50000),
        "consumed balance must be returned to the vintage"
    );
}

/// The target's own (assess-created) unconsumed vintage is removed on delete.
#[test]
fn delete_removes_own_unconsumed_vintage() {
    let env = setup();
    seed_decl(
        &env.storage,
        "1",
        DeclarationType::Ppdg3r,
        h1_2024(),
        indexmap::IndexMap::new(),
        None,
    );
    seed_vintage(&env.storage, "CF-1", dec!(30000), dec!(30000));

    let plan = plan_deletion(&env.storage, "1").unwrap();
    assert!(plan.deletes_vintage);
    execute_deletion(&env.storage, &env.cfg, &plan).unwrap();

    assert!(env.storage.find_carryforward_vintage("CF-1").is_none());
    assert!(env.storage.get_declaration("1").is_none());
}

/// A later-period PPDG-3R exists: deleting an earlier one would dangle its
/// carryforward, so planning bails and nothing is deleted.
#[test]
fn delete_guarded_by_later_ppdg3r() {
    let env = setup();
    seed_decl(
        &env.storage,
        "1",
        DeclarationType::Ppdg3r,
        h1_2024(),
        indexmap::IndexMap::new(),
        None,
    );
    seed_decl(
        &env.storage,
        "2",
        DeclarationType::Ppdg3r,
        h2_2024(),
        indexmap::IndexMap::new(),
        None,
    );

    let err = plan_deletion(&env.storage, "1").unwrap_err();
    assert!(err.to_string().contains("later PPDG-3R"), "got: {err}");
    assert!(env.storage.get_declaration("1").is_some());
    assert!(env.storage.get_declaration("2").is_some());
}

/// The target's own vintage has been (manually) consumed: planning bails.
#[test]
fn delete_guarded_by_consumed_own_vintage() {
    let env = setup();
    seed_decl(
        &env.storage,
        "1",
        DeclarationType::Ppdg3r,
        h1_2024(),
        indexmap::IndexMap::new(),
        None,
    );
    seed_vintage(&env.storage, "CF-1", dec!(50000), dec!(30000));

    let err = plan_deletion(&env.storage, "1").unwrap_err();
    assert!(err.to_string().contains("has been consumed"), "got: {err}");
    assert!(env.storage.get_declaration("1").is_some());
}

/// PP-OPO declarations are independent; deleting one from the middle of the
/// list removes only it and does not touch the ledger.
#[test]
fn delete_ppo_from_middle_of_list() {
    let env = setup();
    seed_decl(
        &env.storage,
        "1",
        DeclarationType::Ppo,
        (d(2024, 1, 5), d(2024, 1, 5)),
        indexmap::IndexMap::new(),
        None,
    );
    seed_decl(
        &env.storage,
        "2",
        DeclarationType::Ppo,
        (d(2024, 3, 10), d(2024, 3, 10)),
        indexmap::IndexMap::new(),
        None,
    );
    seed_decl(
        &env.storage,
        "3",
        DeclarationType::Ppo,
        (d(2024, 5, 5), d(2024, 5, 5)),
        indexmap::IndexMap::new(),
        None,
    );

    let plan = plan_deletion(&env.storage, "2").unwrap();
    assert!(!plan.deletes_vintage);
    execute_deletion(&env.storage, &env.cfg, &plan).unwrap();

    assert!(env.storage.get_declaration("2").is_none());
    assert!(env.storage.get_declaration("1").is_some());
    assert!(env.storage.get_declaration("3").is_some());
    assert!(env.storage.get_carryforward_vintages().is_empty());
}

/// The declaration's XML file, its copy in the output directory, and its
/// attachments directory are all removed from disk.
#[test]
fn delete_removes_files_from_disk() {
    let env = setup();
    let filename = "001-ppdg3r-h1-2024.xml";
    let xml_path = env.storage.declarations_dir().join(filename);
    std::fs::write(&xml_path, "<xml/>").unwrap();
    let output_copy = env.output_dir.join(filename);
    std::fs::write(&output_copy, "<xml/>").unwrap();
    let attachments = env.storage.declarations_dir().join("1");
    std::fs::create_dir_all(&attachments).unwrap();
    std::fs::write(attachments.join("receipt.pdf"), b"pdf").unwrap();

    seed_decl(
        &env.storage,
        "1",
        DeclarationType::Ppdg3r,
        h1_2024(),
        indexmap::IndexMap::new(),
        Some(xml_path.display().to_string()),
    );

    let plan = plan_deletion(&env.storage, "1").unwrap();
    execute_deletion(&env.storage, &env.cfg, &plan).unwrap();

    assert!(!xml_path.exists(), "declaration XML should be deleted");
    assert!(!output_copy.exists(), "output-dir copy should be deleted");
    assert!(!attachments.exists(), "attachments dir should be deleted");
    assert!(env.storage.get_declaration("1").is_none());
}

/// Malformed `carryforward_sources` metadata bails before any ledger mutation.
#[test]
fn delete_bails_on_malformed_metadata() {
    let env = setup();
    let mut meta = indexmap::IndexMap::new();
    meta.insert(
        "carryforward_sources".into(),
        serde_json::json!([{ "vintage_id": "CF-origin" }]), // missing amount_used
    );
    seed_vintage(&env.storage, "CF-origin", dec!(50000), dec!(50000));
    seed_decl(
        &env.storage,
        "1",
        DeclarationType::Ppdg3r,
        h1_2024(),
        meta,
        None,
    );

    let plan = plan_deletion(&env.storage, "1").unwrap();
    let err = execute_deletion(&env.storage, &env.cfg, &plan).unwrap_err();
    assert!(err.to_string().contains("amount_used"), "got: {err}");
    // Nothing mutated: declaration and vintage untouched.
    assert!(env.storage.get_declaration("1").is_some());
    assert_eq!(
        env.storage
            .find_carryforward_vintage("CF-origin")
            .unwrap()
            .remaining_loss_rsd,
        dec!(50000)
    );
}

#[test]
fn delete_unknown_id_errors() {
    let env = setup();
    let err = plan_deletion(&env.storage, "99").unwrap_err();
    assert!(err.to_string().contains("not found"), "got: {err}");
}

/// Sanity on the seeded vintage's expiry (keeps `Datelike` in use).
#[test]
fn seeded_vintage_has_expected_expiry() {
    let env = setup();
    seed_vintage(&env.storage, "CF-x", dec!(1000), dec!(1000));
    let v = env.storage.find_carryforward_vintage("CF-x").unwrap();
    assert_eq!(v.origin_period_end.year() + 5, v.expiration_tax_year);
}
