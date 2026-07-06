use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate};
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::{
    CarryforwardVintage, Currency, Declaration, DeclarationStatus, DeclarationType, Transaction,
    TransactionType, UserConfig,
};
use ibkr_porez::nbs::NBSClient;
use ibkr_porez::regenerate::{execute_regeneration, plan_regeneration};
use ibkr_porez::storage::Storage;
use ibkr_porez::sync::generate_and_save_gains_for_period;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ── Fixtures ────────────────────────────────────────────────

const FAKE_NBS_URL: &str = "http://127.0.0.1:1";

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

struct Env {
    _tmp: tempfile::TempDir,
    storage: Storage,
    cal: HolidayCalendar,
    output_dir: PathBuf,
    cfg: UserConfig,
}

fn setup() -> Env {
    let tmp = tempfile::TempDir::new().unwrap();
    let storage = Storage::with_dir(tmp.path());
    let mut cal = HolidayCalendar::empty();
    cal.set_fallback(true);

    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&output_dir).unwrap();

    let cfg = UserConfig {
        ibkr_token: "tok".into(),
        ibkr_query_id: "qid".into(),
        personal_id: "1234567890123".into(),
        full_name: "Test User".into(),
        address: "Test Street 1".into(),
        city_code: "223".into(),
        phone: "0641234567".into(),
        email: "test@test.com".into(),
        data_dir: None,
        output_folder: Some(output_dir.display().to_string()),
    };

    Env {
        _tmp: tmp,
        storage,
        cal,
        output_dir,
        cfg,
    }
}

fn nbs_offline<'a>(storage: &'a Storage, cal: &'a HolidayCalendar) -> NBSClient<'a> {
    NBSClient::with_base_url(storage, cal, FAKE_NBS_URL).with_retries(1, std::time::Duration::ZERO)
}

fn seed_rates(storage: &Storage, rates: &[(&str, &str, &str)]) {
    let mut map = storage.load_rates();
    for (date, currency, rate) in rates {
        map.insert(format!("{date}_{currency}"), (*rate).to_string());
    }
    storage.write_rates(&map).unwrap();
}

#[allow(clippy::too_many_arguments)]
fn txn(
    id: &str,
    txn_type: TransactionType,
    symbol: &str,
    date: NaiveDate,
    quantity: Decimal,
    price: Decimal,
    amount: Decimal,
    description: &str,
) -> Transaction {
    Transaction {
        transaction_id: id.into(),
        date,
        r#type: txn_type,
        symbol: symbol.into(),
        description: description.into(),
        quantity,
        price,
        amount,
        currency: Currency::USD,
        open_date: None,
        open_price: None,
        exchange_rate: None,
        amount_rsd: None,
    }
}

/// Buy/sell pair producing a 65000 RSD gain in H1 2024 (matches the sync-module
/// carryforward fixtures).
fn seed_gain_txns(storage: &Storage) {
    seed_rates(
        storage,
        &[
            ("2024-01-15", "USD", "100.00"),
            ("2024-03-10", "USD", "110.00"),
        ],
    );
    storage
        .save_transactions(&[
            txn(
                "buy-1",
                TransactionType::Trade,
                "AAPL",
                d(2024, 1, 15),
                dec!(10),
                dec!(100),
                dec!(-1000),
                "",
            ),
            txn(
                "sell-1",
                TransactionType::Trade,
                "AAPL",
                d(2024, 3, 10),
                dec!(-10),
                dec!(150),
                dec!(1500),
                "",
            ),
        ])
        .unwrap();
}

fn seed_vintage(
    storage: &Storage,
    id: &str,
    origin_declaration_id: &str,
    origin_period: (NaiveDate, NaiveDate),
    recognized: Decimal,
    remaining: Decimal,
) {
    storage
        .upsert_carryforward_vintage(CarryforwardVintage {
            id: id.into(),
            origin_declaration_id: origin_declaration_id.into(),
            assessment_reference: None,
            origin_period_start: origin_period.0,
            origin_period_end: origin_period.1,
            recognized_loss_rsd: recognized,
            remaining_loss_rsd: remaining,
            created_at: Local::now().naive_local(),
            expiration_tax_year: origin_period.1.year() + 5,
            notes: None,
        })
        .unwrap();
}

fn seed_decl(
    storage: &Storage,
    id: &str,
    dtype: DeclarationType,
    period: (NaiveDate, NaiveDate),
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
        metadata: indexmap::IndexMap::new(),
        attached_files: indexmap::IndexMap::new(),
    };
    storage.save_declaration(&decl).unwrap();
}

fn h1_2024() -> (NaiveDate, NaiveDate) {
    (d(2024, 1, 1), d(2024, 6, 30))
}
fn h2_2024() -> (NaiveDate, NaiveDate) {
    (d(2024, 7, 1), d(2024, 12, 31))
}
fn h1_2023() -> (NaiveDate, NaiveDate) {
    (d(2023, 1, 1), d(2023, 6, 30))
}

// ── Tests ───────────────────────────────────────────────────

/// Forgot-assessment scenario: the declaration was generated before the prior
/// loss was recognized, so it consumed nothing. After the vintage appears,
/// regenerate rebuilds it with the carryforward applied.
#[test]
fn regenerate_applies_forgotten_carryforward() {
    let env = setup();
    seed_gain_txns(&env.storage);
    let nbs = nbs_offline(&env.storage, &env.cal);

    let first = generate_and_save_gains_for_period(
        &env.storage,
        &nbs,
        &env.cfg,
        &env.cal,
        h1_2024().0,
        h1_2024().1,
        &env.output_dir,
        false,
    )
    .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].metadata["carryforward_used_rsd"], "0.00");
    let target_id = first[0].declaration_id.clone();

    // The prior loss is recognized only now.
    seed_vintage(
        &env.storage,
        "CF-origin",
        "origin",
        h1_2023(),
        dec!(50000),
        dec!(50000),
    );

    let plan = plan_regeneration(&env.storage, &target_id).unwrap();
    assert!(!plan.deletes_vintage);

    let created =
        execute_regeneration(&env.storage, &nbs, &env.cfg, &env.cal, &plan, false).unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].metadata["carryforward_used_rsd"], "50000.00");

    let v = env.storage.find_carryforward_vintage("CF-origin").unwrap();
    assert_eq!(v.remaining_loss_rsd, Decimal::ZERO);

    // Old declaration gone; exactly one PPDG-3R remains.
    assert_eq!(
        env.storage
            .get_declarations(None, Some(&DeclarationType::Ppdg3r))
            .len(),
        1
    );
}

/// The target already consumed a vintage. Regeneration must restore the
/// consumed balance before regenerating, then the new declaration re-consumes
/// it — the ledger must not end double-charged.
#[test]
fn regenerate_reverses_then_reconsumes_carryforward() {
    let env = setup();
    seed_gain_txns(&env.storage);
    let nbs = nbs_offline(&env.storage, &env.cal);

    seed_vintage(
        &env.storage,
        "CF-origin",
        "origin",
        h1_2023(),
        dec!(50000),
        dec!(50000),
    );

    let first = generate_and_save_gains_for_period(
        &env.storage,
        &nbs,
        &env.cfg,
        &env.cal,
        h1_2024().0,
        h1_2024().1,
        &env.output_dir,
        false,
    )
    .unwrap();
    assert_eq!(first[0].metadata["carryforward_used_rsd"], "50000.00");
    assert_eq!(
        env.storage
            .find_carryforward_vintage("CF-origin")
            .unwrap()
            .remaining_loss_rsd,
        Decimal::ZERO
    );
    let target_id = first[0].declaration_id.clone();

    let plan = plan_regeneration(&env.storage, &target_id).unwrap();
    let created =
        execute_regeneration(&env.storage, &nbs, &env.cfg, &env.cal, &plan, false).unwrap();

    // Re-consumption of the restored balance proves the reversal happened: a
    // missing reversal would leave remaining at 0 and used at 0.
    assert_eq!(created[0].metadata["carryforward_used_rsd"], "50000.00");
    assert_eq!(
        env.storage
            .find_carryforward_vintage("CF-origin")
            .unwrap()
            .remaining_loss_rsd,
        Decimal::ZERO
    );
}

/// The target has its own (assess-created) vintage that no later declaration
/// has consumed. Regeneration removes it and flags that assessment data was
/// dropped.
#[test]
fn regenerate_removes_own_unconsumed_vintage() {
    let env = setup();
    seed_gain_txns(&env.storage);
    let nbs = nbs_offline(&env.storage, &env.cal);

    let first = generate_and_save_gains_for_period(
        &env.storage,
        &nbs,
        &env.cfg,
        &env.cal,
        h1_2024().0,
        h1_2024().1,
        &env.output_dir,
        false,
    )
    .unwrap();
    let target_id = first[0].declaration_id.clone();

    // As if `assess` recognized a loss for this declaration.
    seed_vintage(
        &env.storage,
        &format!("CF-{target_id}"),
        &target_id,
        h1_2024(),
        dec!(30000),
        dec!(30000),
    );

    let plan = plan_regeneration(&env.storage, &target_id).unwrap();
    assert!(plan.deletes_vintage);

    execute_regeneration(&env.storage, &nbs, &env.cfg, &env.cal, &plan, false).unwrap();

    assert!(
        env.storage
            .find_carryforward_vintage(&format!("CF-{target_id}"))
            .is_none()
    );
}

/// A later-period PPDG-3R exists: regenerating an earlier one would invalidate
/// the chain, so planning bails and nothing is deleted.
#[test]
fn regenerate_ppdg3r_guarded_by_later_declaration() {
    let env = setup();
    seed_decl(&env.storage, "1", DeclarationType::Ppdg3r, h1_2024(), None);
    seed_decl(&env.storage, "2", DeclarationType::Ppdg3r, h2_2024(), None);

    let err = plan_regeneration(&env.storage, "1").unwrap_err();
    assert!(err.to_string().contains("later PPDG-3R"), "got: {err}");

    assert!(env.storage.get_declaration("1").is_some());
    assert!(env.storage.get_declaration("2").is_some());
}

/// The target's own vintage has been (manually) consumed: planning bails so the
/// user fixes the ledger first.
#[test]
fn regenerate_guarded_by_consumed_vintage() {
    let env = setup();
    seed_decl(&env.storage, "1", DeclarationType::Ppdg3r, h1_2024(), None);
    seed_vintage(
        &env.storage,
        "CF-1",
        "1",
        h1_2024(),
        dec!(50000),
        dec!(30000),
    );

    let err = plan_regeneration(&env.storage, "1").unwrap_err();
    assert!(err.to_string().contains("has been consumed"), "got: {err}");

    assert!(env.storage.get_declaration("1").is_some());
}

/// PP-OPO declarations are independent; regenerating one from the middle of the
/// list deletes and recreates only it, with a fresh non-colliding id, and does
/// not touch the ledger.
#[test]
fn regenerate_ppo_from_middle_of_list() {
    let env = setup();
    seed_rates(
        &env.storage,
        &[
            ("2024-03-10", "USD", "108.00"),
            ("2024-03-17", "USD", "108.00"),
        ],
    );
    env.storage
        .save_transactions(&[
            txn(
                "d1",
                TransactionType::Dividend,
                "VOO",
                d(2024, 3, 10),
                Decimal::ZERO,
                Decimal::ZERO,
                dec!(50.0),
                "VOO.US(US9229083632) Cash Dividend",
            ),
            txn(
                "w1",
                TransactionType::WithholdingTax,
                "VOO",
                d(2024, 3, 17),
                Decimal::ZERO,
                Decimal::ZERO,
                dec!(-7.50),
                "VOO.US(US9229083632) Tax",
            ),
        ])
        .unwrap();

    let day = (d(2024, 3, 10), d(2024, 3, 10));
    seed_decl(
        &env.storage,
        "1",
        DeclarationType::Ppo,
        (d(2024, 1, 5), d(2024, 1, 5)),
        None,
    );
    seed_decl(&env.storage, "2", DeclarationType::Ppo, day, None);
    seed_decl(
        &env.storage,
        "3",
        DeclarationType::Ppo,
        (d(2024, 5, 5), d(2024, 5, 5)),
        None,
    );

    let nbs = nbs_offline(&env.storage, &env.cal);
    let plan = plan_regeneration(&env.storage, "2").unwrap();
    assert!(!plan.deletes_vintage);

    let created =
        execute_regeneration(&env.storage, &nbs, &env.cfg, &env.cal, &plan, true).unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].declaration_id, "4", "new id must not collide");
    assert_eq!(created[0].r#type, DeclarationType::Ppo);

    assert!(env.storage.get_declaration("2").is_none());
    assert!(env.storage.get_declaration("1").is_some());
    assert!(env.storage.get_declaration("3").is_some());
    assert!(env.storage.get_carryforward_vintages().is_empty());
}

/// The declaration's XML file, its copy in the output directory, and its
/// attachments directory are all removed from disk.
#[test]
fn regenerate_removes_files_from_disk() {
    let env = setup();
    // No transactions: the regenerated period has nothing to declare, so no new
    // file is written over the deleted one.
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
        Some(xml_path.display().to_string()),
    );

    let nbs = nbs_offline(&env.storage, &env.cal);
    let plan = plan_regeneration(&env.storage, "1").unwrap();
    let created =
        execute_regeneration(&env.storage, &nbs, &env.cfg, &env.cal, &plan, false).unwrap();
    assert!(created.is_empty(), "no taxable sales -> nothing recreated");

    assert!(!xml_path.exists(), "declaration XML should be deleted");
    assert!(!output_copy.exists(), "output-dir copy should be deleted");
    assert!(!attachments.exists(), "attachments dir should be deleted");
    assert!(env.storage.get_declaration("1").is_none());
}
