use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Duration, Local, NaiveDate};
use tracing::{debug, info, warn};

use crate::config;
use crate::declaration_manager::PURS_NUMBER_KEY;
use crate::fetch;
use crate::holidays::{HolidayCalendar, HolidayError};
use crate::ibkr_flex::IBKRClient;
use crate::models::CarryforwardSource;
#[cfg(test)]
use crate::models::{CarryforwardVintage, Currency, Transaction, TransactionType};
use crate::models::{Declaration, DeclarationStatus, DeclarationType, UserConfig};
use crate::nbs::NBSClient;
use crate::report_gains::generate_gains_report;
use crate::report_income::{
    Amends, Declared, GroupAction, GroupKey, RenderOptions, SourceAmounts, collect_income_groups,
    decide, render_income_report, wait_expires_on,
};
use crate::storage::Storage;

const DEFAULT_LOOKBACK_DAYS: i64 = 45;

#[derive(Default)]
pub struct SyncOptions {
    pub force: bool,
    pub forced_lookback_days: Option<i64>,
}

#[derive(Debug)]
pub struct SyncResult {
    pub created_declarations: Vec<Declaration>,
    pub gains_skipped: bool,
    pub income_skipped: bool,
    pub income_notices: Vec<String>,
    /// Set when the IBKR fetch failed but declarations were still generated
    /// from already-stored transactions.
    pub fetch_error: Option<String>,
    pub end_period: NaiveDate,
}

pub fn run_sync(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    options: &SyncOptions,
    ibkr: &IBKRClient,
) -> Result<SyncResult> {
    validate_config_or_bail(config)?;

    // A flaky IBKR connection must not block declaration generation: if the
    // fetch fails we still generate from already-stored transactions and
    // surface the failure so the daily auto-sync keeps retrying for fresh data.
    let fetch_error = match fetch::fetch_and_import(storage, nbs, config, ibkr) {
        Ok(fetch_result) => {
            info!(
                inserted = fetch_result.inserted,
                updated = fetch_result.updated,
                total = fetch_result.transactions.len(),
                "fetch complete"
            );
            None
        }
        Err(e) => {
            let msg = format!("{e:#}");
            warn!(error = %msg, "IBKR fetch failed; generating from stored transactions");
            Some(msg)
        }
    };

    let mut result = generate_declarations(storage, nbs, config, holidays, options)?;
    result.fetch_error = fetch_error;
    Ok(result)
}

pub fn run_sync_from_xml(
    xml: &str,
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    options: &SyncOptions,
) -> Result<SyncResult> {
    validate_config_or_bail(config)?;

    let fetch_result = fetch::fetch_from_xml(xml, storage, nbs)?;
    info!(
        inserted = fetch_result.inserted,
        updated = fetch_result.updated,
        total = fetch_result.transactions.len(),
        "xml import complete"
    );

    generate_declarations(storage, nbs, config, holidays, options)
}

pub fn run_sync_from_file(
    path: &std::path::Path,
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    options: &SyncOptions,
) -> Result<SyncResult> {
    let xml =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    run_sync_from_xml(&xml, storage, nbs, config, holidays, options)
}

fn generate_declarations(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    options: &SyncOptions,
) -> Result<SyncResult> {
    let end_period = Local::now().date_naive() - Duration::days(1);

    let output_dir = config::get_effective_output_dir_path(config);
    std::fs::create_dir_all(&output_dir)?;

    let mut created_declarations = Vec::new();
    let mut gains_skipped = false;

    match generate_and_save_gains(
        storage,
        nbs,
        config,
        holidays,
        end_period,
        &output_dir,
        options,
    ) {
        Ok(decls) => created_declarations.extend(decls),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("no taxable sales") {
                debug!("no taxable sales in period, skipping gains report");
                gains_skipped = true;
            } else {
                return Err(e.context("PPDG-3R generation failed"));
            }
        }
    }

    let income = generate_and_save_income(
        storage,
        nbs,
        config,
        holidays,
        end_period,
        &output_dir,
        options,
    )
    .context("PP-OPO generation failed")?;

    if income.empty {
        debug!("no undeclared income in period, skipping");
    }
    created_declarations.extend(income.created);

    Ok(SyncResult {
        created_declarations,
        gains_skipped,
        income_skipped: income.empty,
        income_notices: income.notices,
        fetch_error: None,
        end_period,
    })
}

fn validate_config_or_bail(config: &UserConfig) -> Result<()> {
    let issues = config::validate_config(config);
    if !issues.is_empty() {
        bail!("{}", config::format_config_issues(&issues));
    }
    Ok(())
}

fn determine_gains_period(end_period: NaiveDate) -> (NaiveDate, NaiveDate) {
    let year = end_period.year();
    let month = end_period.month();
    if month < 7 {
        let prev = year - 1;
        (
            NaiveDate::from_ymd_opt(prev, 7, 1).unwrap(),
            NaiveDate::from_ymd_opt(prev, 12, 31).unwrap(),
        )
    } else {
        (
            NaiveDate::from_ymd_opt(year, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(year, 6, 30).unwrap(),
        )
    }
}

/// `(scan_start, creation_start, end)`. The two spans are not the same thing:
/// income is declared only inside `[creation_start, end]`, while transactions
/// are read from `scan_start` so an already-declared group can be re-checked.
fn determine_income_period(
    end_period: NaiveDate,
    options: &SyncOptions,
) -> (NaiveDate, NaiveDate, NaiveDate) {
    let lookback = options
        .forced_lookback_days
        .unwrap_or(DEFAULT_LOOKBACK_DAYS);
    let creation_start = end_period - Duration::days(lookback - 1);
    (
        scan_start(end_period, creation_start),
        creation_start,
        end_period,
    )
}

/// Withholding for a distribution of tax year Y is corrected up to the 1042-S
/// filing in March of Y+1, so a full previous calendar year keeps every
/// correctable date in view.
fn scan_start(end_period: NaiveDate, creation_start: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(end_period.year() - 1, 1, 1)
        .unwrap()
        .min(creation_start)
}

fn generate_and_save_gains(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    end_period: NaiveDate,
    output_dir: &Path,
    options: &SyncOptions,
) -> Result<Vec<Declaration>> {
    let (period_start, period_end) = determine_gains_period(end_period);

    let report = generate_gains_report(
        storage,
        nbs,
        config,
        holidays,
        period_start,
        period_end,
        options.force,
    )?;

    if is_duplicate(storage, &report.filename, &DeclarationType::Ppdg3r) {
        debug!(filename = %report.filename, "gains declaration already exists, skipping");
        return Ok(Vec::new());
    }

    let mut metadata = report.metadata();
    let cf_app = &report.carryforward;
    cf_app.apply_to_metadata(&mut metadata);

    // Consume the ledger *before* persisting the declaration: if saving the
    // declaration fails, the consumption is rolled back so a retry doesn't
    // see a half-applied carryforward (the declaration would otherwise not
    // exist yet, `is_duplicate` wouldn't catch it, and the retry would
    // recompute and consume the same vintages again).
    storage
        .apply_carryforward_consumption(&cf_app.sources)
        .with_context(|| storage.io_error_hint())?;

    let decl = match save_declaration(
        storage,
        &report.filename,
        &report.xml_content,
        DeclarationType::Ppdg3r,
        report.period_start,
        report.period_end,
        &report.entries,
        &metadata,
        output_dir,
    ) {
        Ok(decl) => decl,
        Err(e) => {
            let reversal: Vec<CarryforwardSource> = cf_app
                .sources
                .iter()
                .map(|s| CarryforwardSource {
                    vintage_id: s.vintage_id.clone(),
                    amount_used: -s.amount_used,
                })
                .collect();
            if let Err(rollback_err) = storage.apply_carryforward_consumption(&reversal) {
                warn!(error = %rollback_err, "failed to roll back carryforward consumption after save failure");
            }
            return Err(e);
        }
    };

    info!(filename = %report.filename, "created PPDG-3R declaration");
    Ok(vec![decl])
}

struct IncomeOutcome {
    created: Vec<Declaration>,
    notices: Vec<String>,
    /// Nothing created and nothing to report.
    empty: bool,
}

fn generate_and_save_income(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    end_period: NaiveDate,
    output_dir: &Path,
    options: &SyncOptions,
) -> Result<IncomeOutcome> {
    let (scan_start, creation_start, income_end) = determine_income_period(end_period, options);

    let groups = collect_income_groups(&storage.load_transactions(), scan_start, income_end);
    let declared = declared_income_groups(storage);

    // One clock reading for the whole run: a second `Local::now()` here could
    // land past midnight and disagree with `end_period`.
    let today = end_period + Duration::days(1);
    let opts = RenderOptions {
        today,
        force_rates: options.force,
    };

    let mut created = Vec::new();
    let mut notices = Vec::new();
    for group in &groups {
        let amends = match decide(group, declared.lookup(&group.key), creation_start, today) {
            GroupAction::Skip => continue,
            GroupAction::Wait => {
                info!(
                    symbol = %group.key.1,
                    date = %group.key.0,
                    "no withholding tax matched yet; holding the declaration back"
                );
                notices.push(format!(
                    "{} {}: no withholding tax yet, declared with a zero credit from {}",
                    group.key.1,
                    group.key.0.format("%Y-%m-%d"),
                    wait_expires_on(group).format("%Y-%m-%d"),
                ));
                continue;
            }
            GroupAction::Create => None,
            GroupAction::Amend => declared.amends(&group.key),
        };

        match render_income_report(group, amends.as_ref(), nbs, config, holidays, &opts) {
            Ok(report) => {
                let decl = save_declaration(
                    storage,
                    &report.filename,
                    &report.xml_content,
                    DeclarationType::Ppo,
                    report.declaration_date,
                    report.declaration_date,
                    &report.entries,
                    &report.metadata(),
                    output_dir,
                )?;

                // A zero credit reads the same in the document whether the
                // tax was withheld and reversed or never arrived at all;
                // the log is where the two are told apart.
                info!(
                    filename = %report.filename,
                    matched_any_tax = group.matched_any_tax,
                    amends = ?report.amends,
                    "created PP-OPO declaration"
                );
                created.push(decl);
            }
            // Running out of holiday data is not a per-group problem:
            // generating with wrong holiday handling is worse than failing.
            Err(e) if e.downcast_ref::<HolidayError>().is_some() => return Err(e),
            Err(e) => notices.push(format!(
                "{} {}: {e:#}",
                group.key.1,
                group.key.0.format("%Y-%m-%d"),
            )),
        }
    }

    let empty = created.is_empty() && notices.is_empty();
    Ok(IncomeOutcome {
        created,
        notices,
        empty,
    })
}

struct DeclaredGroup {
    declaration_id: String,
    numeric_id: u64,
    source: Option<SourceAmounts>,
    purs_number: Option<String>,
}

struct DeclaredGroups(HashMap<GroupKey, DeclaredGroup>);

impl DeclaredGroups {
    fn lookup(&self, key: &GroupKey) -> Declared<'_> {
        match self.0.get(key) {
            None => Declared::No,
            Some(declared) => match &declared.source {
                Some(source) => Declared::Yes(source),
                None => Declared::YesWithoutRecord,
            },
        }
    }

    fn amends(&self, key: &GroupKey) -> Option<Amends> {
        self.0.get(key).map(|declared| Amends {
            declaration_id: declared.declaration_id.clone(),
            purs_number: declared.purs_number.clone(),
        })
    }
}

/// Every income group that already has a PP-OPO, keyed exactly as
/// `collect_income_groups` keys its groups so the two sides cannot drift apart.
///
/// A group that has been amended holds two declarations under one key; the
/// newest is kept, so the settled amounts are what the next sync compares
/// against and a further amendment references the amendment before it.
fn declared_income_groups(storage: &Storage) -> DeclaredGroups {
    let mut groups: HashMap<GroupKey, DeclaredGroup> = HashMap::new();
    for decl in storage.get_declarations(None, Some(&DeclarationType::Ppo)) {
        let symbol = decl.metadata.get("symbol").and_then(|v| v.as_str());
        let income_type = decl.metadata.get("income_type").and_then(|v| v.as_str());
        let (Some(symbol), Some(income_type)) = (symbol, income_type) else {
            // Nothing else identifies the group, so it reads as undeclared and
            // is declared again; the warning is what points at the edited file.
            warn!(
                declaration_id = %decl.declaration_id,
                "PP-OPO without symbol/income_type metadata, cannot match it to an income group"
            );
            continue;
        };

        let key = (
            decl.period_start,
            symbol.to_uppercase(),
            income_type.to_string(),
        );
        let candidate = DeclaredGroup {
            declaration_id: decl.declaration_id.clone(),
            numeric_id: decl.declaration_id.parse::<u64>().unwrap_or(0),
            source: SourceAmounts::from_metadata(&decl.metadata),
            purs_number: decl
                .metadata
                .get(PURS_NUMBER_KEY)
                .and_then(|v| v.as_str())
                .map(str::to_string),
        };
        match groups.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                if candidate.numeric_id >= slot.get().numeric_id {
                    slot.insert(candidate);
                }
            }
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(candidate);
            }
        }
    }
    DeclaredGroups(groups)
}

fn is_duplicate(storage: &Storage, generator_filename: &str, decl_type: &DeclarationType) -> bool {
    let stem = Path::new(generator_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(generator_filename);

    let existing = storage.get_declarations(None, Some(decl_type));
    existing.iter().any(|d| {
        d.file_path.as_deref().is_some_and(|fp| {
            let existing_stem = Path::new(fp)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            existing_stem.ends_with(stem)
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn save_declaration<T: serde::Serialize>(
    storage: &Storage,
    generator_filename: &str,
    xml_content: &str,
    decl_type: DeclarationType,
    period_start: NaiveDate,
    period_end: NaiveDate,
    entries: &[T],
    metadata: &indexmap::IndexMap<String, serde_json::Value>,
    output_dir: &Path,
) -> Result<Declaration> {
    let existing = storage.get_declarations(None, None);
    // max+1, not len+1: after a declaration is deleted from the middle of the
    // list (delete PP-OPO case) len+1 could collide with a surviving id and
    // silently overwrite it in `storage.save_declaration`.
    let next_id = existing
        .iter()
        .filter_map(|d| d.declaration_id.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let id_str = next_id.to_string();
    let proper_filename = format!("{next_id:03}-{generator_filename}");

    let decl_path = storage.declarations_dir().join(&proper_filename);
    std::fs::write(&decl_path, xml_content).with_context(|| storage.io_error_hint())?;

    let output_path = output_dir.join(&proper_filename);
    std::fs::write(&output_path, xml_content).with_context(|| {
        format!(
            "Failed to write to output directory: {}",
            output_dir.display()
        )
    })?;

    let report_data: Vec<serde_json::Value> = entries
        .iter()
        .filter_map(|e| serde_json::to_value(e).ok())
        .collect();

    let decl = Declaration {
        declaration_id: id_str,
        r#type: decl_type,
        status: DeclarationStatus::Draft,
        period_start,
        period_end,
        created_at: Local::now().naive_local(),
        submitted_at: None,
        paid_at: None,
        file_path: Some(decl_path.display().to_string()),
        xml_content: Some(xml_content.to_string()),
        report_data: Some(report_data),
        metadata: metadata.clone(),
        attached_files: indexmap::IndexMap::new(),
    };

    storage
        .save_declaration(&decl)
        .with_context(|| storage.io_error_hint())?;
    Ok(decl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    #[test]
    fn test_gains_period_h1() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        let (start, end) = determine_gains_period(date);
        assert_eq!(start, NaiveDate::from_ymd_opt(2025, 7, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
    }

    #[test]
    fn test_gains_period_h2() {
        let date = NaiveDate::from_ymd_opt(2025, 9, 1).unwrap();
        let (start, end) = determine_gains_period(date);
        assert_eq!(start, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2025, 6, 30).unwrap());
    }

    #[test]
    fn income_period_is_fixed_window() {
        let end = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let opts = SyncOptions::default();

        let (_, creation_start, period_end) = determine_income_period(end, &opts);
        assert_eq!(period_end, end);
        assert_eq!(creation_start, end - Duration::days(44));
    }

    #[test]
    fn test_forced_lookback_overrides_start() {
        let end = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let opts = SyncOptions {
            force: false,
            forced_lookback_days: Some(90),
        };

        let (_, creation_start, _) = determine_income_period(end, &opts);
        assert_eq!(creation_start, end - Duration::days(89));
    }

    #[test]
    fn scan_horizon_is_previous_calendar_year() {
        let opts = SyncOptions::default();
        let jan_1_2025 = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

        let (scan_start, _, _) =
            determine_income_period(NaiveDate::from_ymd_opt(2026, 7, 26).unwrap(), &opts);
        assert_eq!(scan_start, jan_1_2025);

        let (scan_start, _, _) =
            determine_income_period(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(), &opts);
        assert_eq!(scan_start, jan_1_2025);
    }

    #[test]
    fn scan_horizon_follows_a_deeper_lookback() {
        let end = NaiveDate::from_ymd_opt(2026, 7, 26).unwrap();
        let opts = SyncOptions {
            force: false,
            forced_lookback_days: Some(3650),
        };

        let (scan_start, creation_start, _) = determine_income_period(end, &opts);
        assert_eq!(scan_start, creation_start);
        assert!(scan_start < NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
    }

    #[test]
    fn test_is_duplicate_stem_match() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());

        let decl = Declaration {
            declaration_id: "1".into(),
            r#type: DeclarationType::Ppdg3r,
            status: DeclarationStatus::Draft,
            period_start: NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
            period_end: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            created_at: Local::now().naive_local(),
            submitted_at: None,
            paid_at: None,
            file_path: Some("001-ppdg3r-h2-2025.xml".into()),
            xml_content: None,
            report_data: None,
            metadata: indexmap::IndexMap::new(),
            attached_files: indexmap::IndexMap::new(),
        };
        storage.save_declaration(&decl).unwrap();

        assert!(is_duplicate(
            &storage,
            "ppdg3r-h2-2025.xml",
            &DeclarationType::Ppdg3r
        ));
        assert!(!is_duplicate(
            &storage,
            "ppdg3r-h1-2026.xml",
            &DeclarationType::Ppdg3r
        ));
    }

    #[test]
    fn test_is_duplicate_different_type_no_match() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());

        let decl = Declaration {
            declaration_id: "1".into(),
            r#type: DeclarationType::Ppdg3r,
            status: DeclarationStatus::Draft,
            period_start: NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
            period_end: NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            created_at: Local::now().naive_local(),
            submitted_at: None,
            paid_at: None,
            file_path: Some("001-ppdg3r-h2-2025.xml".into()),
            xml_content: None,
            report_data: None,
            metadata: indexmap::IndexMap::new(),
            attached_files: indexmap::IndexMap::new(),
        };
        storage.save_declaration(&decl).unwrap();

        assert!(!is_duplicate(
            &storage,
            "ppdg3r-h2-2025.xml",
            &DeclarationType::Ppo
        ));
    }

    fn valid_config() -> UserConfig {
        UserConfig {
            ibkr_token: "tok".into(),
            ibkr_query_id: "qid".into(),
            personal_id: "1234567890123".into(),
            full_name: "Test User".into(),
            address: "Test Address 1".into(),
            city_code: "11000".into(),
            phone: "0611234567".into(),
            email: "test@example.org".into(),
            ..UserConfig::default()
        }
    }

    fn ibkr_xml_no_trades() -> &'static str {
        r"<FlexQueryResponse>
          <FlexStatements>
            <FlexStatement>
              <Trades />
              <CashTransactions />
            </FlexStatement>
          </FlexStatements>
        </FlexQueryResponse>"
    }

    fn send_request_matcher() -> mockito::Matcher {
        mockito::Matcher::Regex(r"^/SendRequest\?".into())
    }

    fn setup_ibkr_mock(server: &mut mockito::Server, xml: &str) -> (mockito::Mock, mockito::Mock) {
        let req = server
            .mock("GET", send_request_matcher())
            .with_status(200)
            .with_body(format!(
                "<FlexStatementResponse>\
                   <Status>Success</Status>\
                   <ReferenceCode>REF1</ReferenceCode>\
                   <Url>{}/GetStatement</Url>\
                 </FlexStatementResponse>",
                server.url()
            ))
            .create();
        let get = server
            .mock("GET", mockito::Matcher::Regex(r"^/GetStatement\?".into()))
            .with_status(200)
            .with_body(xml)
            .create();
        (req, get)
    }

    #[test]
    fn run_sync_invalid_config_is_rejected() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let cal = crate::holidays::HolidayCalendar::load_embedded();
        let nbs = crate::nbs::NBSClient::with_base_url(&storage, &cal, "http://127.0.0.1:1");
        let ibkr = IBKRClient::with_base_url("tok", "qid", "http://127.0.0.1:1");
        let cfg = UserConfig::default();
        let opts = SyncOptions::default();

        let result = run_sync(&storage, &nbs, &cfg, &cal, &opts, &ibkr);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Configuration"));
    }

    #[test]
    fn run_sync_no_trades_skips_both() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let cal = crate::holidays::HolidayCalendar::load_embedded();
        let mut cfg = valid_config();
        cfg.output_folder = Some(tmp.path().join("output").display().to_string());

        let mut ibkr_server = mockito::Server::new();
        let (req_m, get_m) = setup_ibkr_mock(&mut ibkr_server, ibkr_xml_no_trades());
        let ibkr = IBKRClient::with_base_url("tok", "qid", &ibkr_server.url());

        let mut nbs_server = mockito::Server::new();
        let _nbs_mock = nbs_server
            .mock("GET", mockito::Matcher::Any)
            .with_status(404)
            .create();
        let nbs = crate::nbs::NBSClient::with_base_url(&storage, &cal, &nbs_server.url());

        let opts = SyncOptions::default();
        let result = run_sync(&storage, &nbs, &cfg, &cal, &opts, &ibkr).unwrap();

        assert!(result.created_declarations.is_empty());
        assert!(result.gains_skipped);
        assert!(result.income_skipped);
        req_m.assert();
        get_m.assert();
    }

    #[test]
    fn run_sync_ibkr_failure_still_generates() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let cal = crate::holidays::HolidayCalendar::load_embedded();
        let mut cfg = valid_config();
        cfg.output_folder = Some(tmp.path().join("output").display().to_string());

        let mut ibkr_server = mockito::Server::new();
        let mock = ibkr_server
            .mock("GET", send_request_matcher())
            .with_status(500)
            .expect_at_least(1)
            .create();
        let ibkr = IBKRClient::with_base_url("tok", "qid", &ibkr_server.url());

        let nbs = crate::nbs::NBSClient::with_base_url(&storage, &cal, "http://127.0.0.1:1");

        let opts = SyncOptions::default();
        // A failed IBKR fetch must not abort generation: run_sync still returns
        // Ok, surfaces the fetch failure, and generates from stored data.
        let result = run_sync(&storage, &nbs, &cfg, &cal, &opts, &ibkr).unwrap();
        assert!(result.fetch_error.is_some());
        assert!(result.created_declarations.is_empty());
        mock.assert();
    }

    #[test]
    fn validate_config_or_bail_passes_valid() {
        let cfg = valid_config();
        assert!(validate_config_or_bail(&cfg).is_ok());
    }

    #[test]
    fn validate_config_or_bail_rejects_empty() {
        let cfg = UserConfig::default();
        let err = validate_config_or_bail(&cfg).unwrap_err();
        assert!(err.to_string().contains("Configuration"));
    }

    // -----------------------------------------------------------------
    // Carryforward consumption during gains declaration creation
    // -----------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn make_txn(
        id: &str,
        symbol: &str,
        date: &str,
        quantity: rust_decimal::Decimal,
        price: rust_decimal::Decimal,
        amount: rust_decimal::Decimal,
    ) -> Transaction {
        Transaction {
            transaction_id: id.to_string(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            r#type: TransactionType::Trade,
            symbol: symbol.to_string(),
            description: String::new(),
            quantity,
            price,
            amount,
            currency: Currency::USD,
            open_date: None,
            open_price: None,
            exchange_rate: None,
            amount_rsd: None,
            action_id: None,
        }
    }

    fn seed_rates(storage: &Storage, rates: &[(&str, &str)]) {
        let mut rate_map = storage.load_rates();
        for (date, rate) in rates {
            rate_map.insert(format!("{date}_USD"), (*rate).to_string());
        }
        storage.write_rates(&rate_map).unwrap();
    }

    fn seed_carryforward_vintage(storage: &Storage, id: &str, remaining: rust_decimal::Decimal) {
        storage
            .upsert_carryforward_vintage(CarryforwardVintage {
                id: id.into(),
                origin_declaration_id: "origin".into(),
                assessment_reference: None,
                origin_period_start: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
                origin_period_end: NaiveDate::from_ymd_opt(2023, 6, 30).unwrap(),
                recognized_loss_rsd: remaining,
                remaining_loss_rsd: remaining,
                created_at: Local::now().naive_local(),
                expiration_tax_year: 2028,
                notes: None,
            })
            .unwrap();
    }

    fn nbs_offline<'a>(
        storage: &'a Storage,
        cal: &'a crate::holidays::HolidayCalendar,
    ) -> crate::nbs::NBSClient<'a> {
        crate::nbs::NBSClient::with_base_url(storage, cal, "http://127.0.0.1:1")
            .with_retries(1, std::time::Duration::ZERO)
    }

    fn carryforward_test_setup() -> (tempfile::TempDir, Storage, crate::holidays::HolidayCalendar) {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let mut cal = crate::holidays::HolidayCalendar::empty();
        cal.set_fallback(true);

        seed_rates(
            &storage,
            &[("2024-01-15", "100.00"), ("2024-03-10", "110.00")],
        );

        let txns = vec![
            make_txn(
                "buy-1",
                "AAPL",
                "2024-01-15",
                dec!(10),
                dec!(100),
                dec!(-1000),
            ),
            make_txn(
                "sell-1",
                "AAPL",
                "2024-03-10",
                dec!(-10),
                dec!(150),
                dec!(1500),
            ),
        ];
        storage.save_transactions(&txns).unwrap();

        (tmp, storage, cal)
    }

    #[test]
    fn generate_and_save_gains_consumes_carryforward() {
        let (tmp, storage, cal) = carryforward_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let cfg = valid_config();
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();
        let opts = SyncOptions::default();

        seed_carryforward_vintage(&storage, "CF-origin", dec!(50000));

        let end_period = NaiveDate::from_ymd_opt(2024, 8, 1).unwrap();
        let decls =
            generate_and_save_gains(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();

        assert_eq!(decls.len(), 1);
        let decl = &decls[0];
        assert_eq!(decl.metadata["tax_base_rsd"], "65000.00");
        assert_eq!(decl.metadata["opening_carryforward_rsd"], "50000.00");
        assert_eq!(decl.metadata["carryforward_used_rsd"], "50000.00");
        assert_eq!(decl.metadata["closing_carryforward_rsd"], "0.00");
        assert_eq!(decl.metadata["adjusted_tax_base_rsd"], "15000.00");
        assert_eq!(decl.metadata["estimated_tax_rsd"], "2250.00");

        let sources = decl.metadata["carryforward_sources"].as_array().unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["vintage_id"], "CF-origin");
        assert_eq!(sources[0]["amount_used"], "50000.00");

        let vintage = storage.find_carryforward_vintage("CF-origin").unwrap();
        assert_eq!(vintage.remaining_loss_rsd, Decimal::ZERO);

        let xml = decl.xml_content.as_deref().unwrap();
        assert!(xml.contains("<ns1:DeklarisanoKapitalniGubiciRanijihGodina>"));
        assert!(xml.contains(
            "<ns1:IznosUtvrdjenogKapitalnogGubitka>50000.00</ns1:IznosUtvrdjenogKapitalnogGubitka>"
        ));
        assert!(xml.contains(
            "<ns1:KapitalniGubitakIzRanijihGodina>50000.00</ns1:KapitalniGubitakIzRanijihGodina>"
        ));
        assert!(xml.contains("<ns1:Osnovica>15000.00</ns1:Osnovica>"));
        assert!(xml.contains("<ns1:PorezZaUplatu>2250.00</ns1:PorezZaUplatu>"));
    }

    #[test]
    fn generate_and_save_gains_idempotent_consumption() {
        let (tmp, storage, cal) = carryforward_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let cfg = valid_config();
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();
        let opts = SyncOptions::default();

        seed_carryforward_vintage(&storage, "CF-origin", dec!(50000));

        let end_period = NaiveDate::from_ymd_opt(2024, 8, 1).unwrap();
        let first =
            generate_and_save_gains(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(first.len(), 1);

        let second =
            generate_and_save_gains(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert!(
            second.is_empty(),
            "second sync should hit is_duplicate and skip"
        );

        let vintage = storage.find_carryforward_vintage("CF-origin").unwrap();
        assert_eq!(vintage.remaining_loss_rsd, Decimal::ZERO);
    }

    #[test]
    fn generate_and_save_gains_preserves_historical_snapshot() {
        let (tmp, storage, cal) = carryforward_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let cfg = valid_config();
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();
        let opts = SyncOptions::default();

        seed_carryforward_vintage(&storage, "CF-origin", dec!(50000));

        // First period (H1 2024): consumes the full carryforward balance.
        let end_period_1 = NaiveDate::from_ymd_opt(2024, 8, 1).unwrap();
        let first =
            generate_and_save_gains(&storage, &nbs, &cfg, &cal, end_period_1, &output_dir, &opts)
                .unwrap();
        assert_eq!(first.len(), 1);
        let decl_1_id = first[0].declaration_id.clone();

        // Second period (H2 2024): independent gain, no carryforward left.
        seed_rates(
            &storage,
            &[("2024-08-01", "100.00"), ("2024-10-01", "105.00")],
        );
        let mut txns = storage.load_transactions();
        txns.push(make_txn(
            "buy-2",
            "MSFT",
            "2024-08-01",
            dec!(5),
            dec!(200),
            dec!(-1000),
        ));
        txns.push(make_txn(
            "sell-2",
            "MSFT",
            "2024-10-01",
            dec!(-5),
            dec!(220),
            dec!(1100),
        ));
        storage.write_transactions(&txns).unwrap();

        let end_period_2 = NaiveDate::from_ymd_opt(2025, 3, 1).unwrap();
        let second =
            generate_and_save_gains(&storage, &nbs, &cfg, &cal, end_period_2, &output_dir, &opts)
                .unwrap();
        assert_eq!(second.len(), 1);

        // The first declaration's carryforward snapshot must be unchanged.
        let decl_1 = storage.get_declaration(&decl_1_id).unwrap();
        assert_eq!(decl_1.metadata["carryforward_used_rsd"], "50000.00");
        assert_eq!(decl_1.metadata["closing_carryforward_rsd"], "0.00");

        // The second declaration sees no remaining carryforward.
        assert_eq!(second[0].metadata["opening_carryforward_rsd"], "0.00");
        assert_eq!(second[0].metadata["carryforward_used_rsd"], "0.00");
    }

    #[test]
    fn save_declaration_uses_max_id_plus_one_after_delete() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        let seed = |id: &str| Declaration {
            declaration_id: id.into(),
            r#type: DeclarationType::Ppo,
            status: DeclarationStatus::Draft,
            period_start: NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
            period_end: NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
            created_at: Local::now().naive_local(),
            submitted_at: None,
            paid_at: None,
            file_path: None,
            xml_content: None,
            report_data: None,
            metadata: indexmap::IndexMap::new(),
            attached_files: indexmap::IndexMap::new(),
        };
        storage.save_declaration(&seed("1")).unwrap();
        storage.save_declaration(&seed("2")).unwrap();
        storage.save_declaration(&seed("3")).unwrap();

        storage.delete_declaration("2").unwrap();

        let entries: Vec<serde_json::Value> = Vec::new();
        let new = save_declaration(
            &storage,
            "ppo-2025-03-10.xml",
            "<xml/>",
            DeclarationType::Ppo,
            NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
            NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
            &entries,
            &indexmap::IndexMap::new(),
            &output_dir,
        )
        .unwrap();

        assert_eq!(new.declaration_id, "4", "new id must be max+1, not len+1");
        // declaration "3" must be untouched by the len+1 collision that would
        // otherwise overwrite it.
        assert!(storage.get_declaration("3").is_some());
        assert_eq!(storage.get_declarations(None, None).len(), 3);
    }

    // -----------------------------------------------------------------
    // Income declaration creation
    // -----------------------------------------------------------------

    fn income_txn(
        id: &str,
        r#type: TransactionType,
        symbol: &str,
        date: &str,
        amount: Decimal,
        description: &str,
    ) -> Transaction {
        Transaction {
            transaction_id: id.to_string(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            r#type,
            symbol: symbol.to_string(),
            description: description.to_string(),
            quantity: Decimal::ZERO,
            price: Decimal::ZERO,
            amount,
            currency: Currency::USD,
            open_date: None,
            open_price: None,
            exchange_rate: None,
            amount_rsd: None,
            action_id: None,
        }
    }

    /// A withholding row of the same distribution as [`dividend_rows`].
    fn withholding_row(id: &str, symbol: &str, date: &str, amount: Decimal) -> Transaction {
        income_txn(
            id,
            TransactionType::WithholdingTax,
            symbol,
            date,
            amount,
            &format!("{symbol}(US0000000000) CASH DIVIDEND USD 1.00 PER SHARE - US TAX"),
        )
    }

    /// A dividend and its withholding row, matched by the `PER SHARE` prefix.
    fn dividend_rows(symbol: &str, date: &str, gross: Decimal, tax: Decimal) -> Vec<Transaction> {
        vec![
            income_txn(
                &format!("d-{symbol}-{date}"),
                TransactionType::Dividend,
                symbol,
                date,
                gross,
                &format!(
                    "{symbol}(US0000000000) CASH DIVIDEND USD 1.00 PER SHARE (Ordinary Dividend)"
                ),
            ),
            withholding_row(&format!("w-{symbol}-{date}"), symbol, date, tax),
        ]
    }

    fn income_test_setup() -> (
        tempfile::TempDir,
        Storage,
        crate::holidays::HolidayCalendar,
        UserConfig,
        std::path::PathBuf,
    ) {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let mut cal = crate::holidays::HolidayCalendar::empty();
        cal.set_fallback(true);

        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();
        let mut cfg = valid_config();
        cfg.output_folder = Some(output_dir.display().to_string());

        (tmp, storage, cal, cfg, output_dir)
    }

    fn declared_ppo(
        id: &str,
        symbol: &str,
        date: NaiveDate,
        file_path: Option<String>,
    ) -> Declaration {
        let mut metadata = indexmap::IndexMap::new();
        metadata.insert("symbol".to_string(), symbol.into());
        metadata.insert("income_type".to_string(), "dividend".into());
        Declaration {
            declaration_id: id.into(),
            r#type: DeclarationType::Ppo,
            status: DeclarationStatus::Draft,
            period_start: date,
            period_end: date,
            created_at: Local::now().naive_local(),
            submitted_at: None,
            paid_at: None,
            file_path,
            xml_content: None,
            report_data: None,
            metadata,
            attached_files: indexmap::IndexMap::new(),
        }
    }

    /// A PP-OPO carrying the source amounts a declaration produced under these
    /// rules records -- the ones an amendment is decided by.
    fn declared_ppo_with_source(
        id: &str,
        symbol: &str,
        date: NaiveDate,
        gross_ccy: &str,
        tax_ccy: &str,
    ) -> Declaration {
        let mut decl = declared_ppo(id, symbol, date, None);
        SourceAmounts {
            gross_ccy: gross_ccy.into(),
            tax_ccy: tax_ccy.into(),
            currency: "USD".into(),
        }
        .to_metadata(&mut decl.metadata);
        decl
    }

    #[test]
    fn late_arriving_income_gets_declared_on_next_sync() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        seed_rates(&storage, &[("2026-03-01", "100.00")]);

        let first =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert!(first.empty, "nothing stored yet");

        // The same window, and a date the first run already scanned past.
        storage
            .save_transactions(&dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15)))
            .unwrap();

        let second =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(second.created.len(), 1);
        assert_eq!(second.created[0].metadata["symbol"], "VOO");
    }

    #[test]
    fn missing_rate_group_does_not_block_others() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        // Only VOO's date has a rate; MSFT's is beyond the NBS lookback from it.
        seed_rates(&storage, &[("2026-03-01", "100.00")]);

        let mut txns = dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15));
        txns.extend(dividend_rows("MSFT", "2026-02-05", dec!(50), dec!(-7.5)));
        storage.save_transactions(&txns).unwrap();

        let outcome =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();

        assert_eq!(outcome.created.len(), 1);
        assert_eq!(outcome.created[0].metadata["symbol"], "VOO");
        assert_eq!(outcome.notices.len(), 1);
        assert!(outcome.notices[0].contains("MSFT"));
    }

    #[test]
    fn deleted_income_declaration_is_regenerated() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        seed_rates(&storage, &[("2026-03-01", "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15)))
            .unwrap();

        let first =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(first.created.len(), 1);

        let second =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert!(second.empty, "an existing declaration is not repeated");

        storage
            .delete_declaration(&first.created[0].declaration_id)
            .unwrap();

        let third =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(third.created.len(), 1);
    }

    #[test]
    fn declaration_without_file_path_is_not_regenerated() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let income_date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        seed_rates(&storage, &[("2026-03-01", "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15)))
            .unwrap();

        storage
            .save_declaration(&declared_ppo("1", "VOO", income_date, None))
            .unwrap();

        let outcome =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert!(
            outcome.empty,
            "the group is keyed by metadata, not by a file name"
        );
    }

    #[test]
    fn no_watermark_is_written() {
        let (_tmp, storage, cal, cfg, _output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        // `generate_declarations` reads the clock, so the income has to be
        // dated relative to it to land inside the creation window.
        let income_date = (Local::now().date_naive() - Duration::days(3))
            .format("%Y-%m-%d")
            .to_string();
        seed_rates(&storage, &[(&income_date, "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", &income_date, dec!(100), dec!(-15)))
            .unwrap();

        let result = generate_declarations(&storage, &nbs, &cfg, &cal, &opts).unwrap();
        assert_eq!(result.created_declarations.len(), 1);
        assert!(storage.get_last_declaration_date().is_none());
    }

    #[test]
    fn declaration_without_source_amounts_is_never_amended() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let income_date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        seed_rates(&storage, &[("2026-03-01", "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15)))
            .unwrap();

        // A declaration that records no source amounts of its own is outside
        // the domain these rules are defined over.
        storage
            .save_declaration(&declared_ppo("1", "VOO", income_date, None))
            .unwrap();

        storage
            .save_transactions(&[withholding_row("w-rev", "VOO", "2026-03-01", dec!(15))])
            .unwrap();

        let outcome =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert!(outcome.empty, "nothing may be reconstructed for it");
    }

    #[test]
    fn income_outside_creation_window_is_not_declared_but_is_compared() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        // Inside the scan horizon (1 Jan 2025), far outside the 45-day window.
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let income_date = NaiveDate::from_ymd_opt(2025, 7, 2).unwrap();
        seed_rates(&storage, &[("2025-07-02", "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", "2025-07-02", dec!(100), dec!(-15)))
            .unwrap();

        let undeclared =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert!(undeclared.empty, "outside the window nothing is created");

        storage
            .save_declaration(&declared_ppo_with_source(
                "1",
                "VOO",
                income_date,
                "100.00",
                "15.00",
            ))
            .unwrap();
        storage
            .save_transactions(&[withholding_row("w-rev", "VOO", "2025-07-02", dec!(15))])
            .unwrap();

        let amended =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(amended.created.len(), 1);
        assert_eq!(amended.created[0].metadata["amends"], "1");
        assert_eq!(amended.created[0].metadata["foreign_tax_paid_ccy"], "0.00");
    }

    #[test]
    fn late_reversal_produces_amendment() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        seed_rates(&storage, &[("2026-03-01", "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15)))
            .unwrap();

        let first =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(first.created.len(), 1);
        assert_eq!(first.created[0].metadata["foreign_tax_paid_ccy"], "15.00");

        // The reversal carries the date of the distribution it reverses; only
        // the row is late, so the amendment comes from the comparison and not
        // from the row looking new.
        storage
            .save_transactions(&[withholding_row("w-rev", "VOO", "2026-03-01", dec!(15))])
            .unwrap();

        let second =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(second.created.len(), 1);
        let amendment = &second.created[0];
        assert_eq!(
            amendment.metadata["amends"],
            first.created[0].declaration_id
        );
        assert_eq!(amendment.metadata["foreign_tax_paid_ccy"], "0.00");
        assert!(
            amendment.file_path.as_ref().unwrap().contains("izmena"),
            "an amendment must not collide with the original on disk"
        );
        assert!(
            amendment
                .xml_content
                .as_ref()
                .unwrap()
                .contains("<ns1:VrstaIzmenePrijave>1</ns1:VrstaIzmenePrijave>")
        );
    }

    #[test]
    fn amendment_is_not_regenerated_on_the_next_sync() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        seed_rates(&storage, &[("2026-03-01", "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15)))
            .unwrap();

        generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
            .unwrap();
        storage
            .save_transactions(&[withholding_row("w-rev", "VOO", "2026-03-01", dec!(15))])
            .unwrap();

        let amended =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(amended.created.len(), 1);

        // The comparison is now against the amendment, not the original.
        let again =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert!(again.empty, "a settled group produces nothing further");
    }

    #[test]
    fn second_change_produces_a_second_amendment_referencing_the_first() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        seed_rates(&storage, &[("2026-03-01", "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15)))
            .unwrap();

        let first =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();

        // The IXUS shape: -X, then +X, then -Y.
        storage
            .save_transactions(&[withholding_row("w-rev", "VOO", "2026-03-01", dec!(15))])
            .unwrap();
        let amendment =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(amendment.created.len(), 1);

        storage
            .save_transactions(&[withholding_row("w-new", "VOO", "2026-03-01", dec!(-12))])
            .unwrap();
        let second =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert_eq!(second.created.len(), 1);
        assert_eq!(
            second.created[0].metadata["amends"], amendment.created[0].declaration_id,
            "an amendment amends the newest declaration of its group"
        );
        assert_ne!(
            second.created[0].metadata["amends"],
            first.created[0].declaration_id
        );
        assert_eq!(second.created[0].metadata["foreign_tax_paid_ccy"], "12.00");
    }

    #[test]
    fn amendment_carries_the_recorded_purs_number() {
        let (_tmp, storage, cal, cfg, output_dir) = income_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let opts = SyncOptions::default();
        let end_period = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        seed_rates(&storage, &[("2026-03-01", "100.00")]);
        storage
            .save_transactions(&dividend_rows("VOO", "2026-03-01", dec!(100), dec!(-15)))
            .unwrap();

        let first =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        crate::declaration_manager::DeclarationManager::new(&storage)
            .submit_with_number(&[&first.created[0].declaration_id], Some("1234567890"))
            .unwrap();

        storage
            .save_transactions(&[withholding_row("w-rev", "VOO", "2026-03-01", dec!(15))])
            .unwrap();
        let amended =
            generate_and_save_income(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts)
                .unwrap();
        assert!(
            amended.created[0]
                .xml_content
                .as_ref()
                .unwrap()
                .contains("<ns1:IdentifikatorPrijave>1234567890</ns1:IdentifikatorPrijave>")
        );
    }

    #[test]
    fn generate_and_save_gains_rolls_back_consumption_on_save_failure() {
        let (tmp, storage, cal) = carryforward_test_setup();
        let nbs = nbs_offline(&storage, &cal);
        let cfg = valid_config();
        // Intentionally not created: save_declaration fails writing here,
        // after the ledger consumption has already been applied.
        let output_dir = tmp.path().join("missing-output");
        let opts = SyncOptions::default();

        seed_carryforward_vintage(&storage, "CF-origin", dec!(50000));

        let end_period = NaiveDate::from_ymd_opt(2024, 8, 1).unwrap();
        let result =
            generate_and_save_gains(&storage, &nbs, &cfg, &cal, end_period, &output_dir, &opts);
        assert!(result.is_err());

        let vintage = storage.find_carryforward_vintage("CF-origin").unwrap();
        assert_eq!(vintage.remaining_loss_rsd, dec!(50000));
        assert!(storage.get_declarations(None, None).is_empty());
    }
}
