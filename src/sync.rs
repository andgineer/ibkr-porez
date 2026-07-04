use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Duration, Local, NaiveDate};
use tracing::{debug, info, warn};

use crate::config;
use crate::fetch;
use crate::holidays::HolidayCalendar;
use crate::ibkr_flex::IBKRClient;
use crate::models::CarryforwardSource;
#[cfg(test)]
use crate::models::{CarryforwardVintage, Currency, Transaction, TransactionType};
use crate::models::{Declaration, DeclarationStatus, DeclarationType, UserConfig};
use crate::nbs::NBSClient;
use crate::report_gains::generate_gains_report;
use crate::report_income::generate_income_reports;
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
    pub income_error: Option<String>,
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

    let fetch_result = fetch::fetch_and_import(storage, nbs, config, ibkr)?;
    info!(
        inserted = fetch_result.inserted,
        updated = fetch_result.updated,
        total = fetch_result.transactions.len(),
        "fetch complete"
    );

    generate_declarations(storage, nbs, config, holidays, options)
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
    let mut income_skipped = false;
    let mut income_error = None;

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

    match generate_and_save_income(
        storage,
        nbs,
        config,
        holidays,
        end_period,
        &output_dir,
        options,
    ) {
        Ok(IncomeOutcome::Created(decls)) => created_declarations.extend(decls),
        Ok(IncomeOutcome::NoIncome) => {
            debug!("no income in period, skipping");
            income_skipped = true;
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("no NBS exchange rate") {
                debug!(error = %e, "income report NBS error collected");
                income_error = Some(msg);
            } else if msg.contains("withholding tax") && !options.force {
                warn!(error = %e, "missing withholding tax; use --force to override");
                return Err(e);
            } else {
                return Err(e.context("PP-OPO generation failed"));
            }
        }
    }

    let current_last = storage.get_last_declaration_date();
    if current_last.is_none_or(|d| d < end_period) {
        storage
            .set_last_declaration_date(end_period)
            .with_context(|| storage.io_error_hint())?;
        debug!(%end_period, "updated last_declaration_date");
    }

    Ok(SyncResult {
        created_declarations,
        gains_skipped,
        income_skipped,
        income_error,
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

fn determine_income_period(
    storage: &Storage,
    end_period: NaiveDate,
    options: &SyncOptions,
) -> Option<(NaiveDate, NaiveDate)> {
    if let Some(lookback) = options.forced_lookback_days {
        let start = end_period - Duration::days(lookback - 1);
        return Some((start, end_period));
    }

    let last = storage.get_last_declaration_date();
    let start = match last {
        Some(d) => d + Duration::days(1),
        None => end_period - Duration::days(DEFAULT_LOOKBACK_DAYS - 1),
    };

    if start > end_period {
        return None;
    }
    Some((start, end_period))
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

enum IncomeOutcome {
    Created(Vec<Declaration>),
    NoIncome,
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
    let Some((income_start, income_end)) = determine_income_period(storage, end_period, options)
    else {
        debug!("income period is empty, skipping");
        return Ok(IncomeOutcome::NoIncome);
    };

    let reports = generate_income_reports(
        storage,
        nbs,
        config,
        holidays,
        income_start,
        income_end,
        options.force,
    )?;

    if reports.is_empty() {
        return Ok(IncomeOutcome::NoIncome);
    }

    let mut created = Vec::new();
    for report in &reports {
        if is_duplicate(storage, &report.filename, &DeclarationType::Ppo) {
            debug!(filename = %report.filename, "income declaration already exists, skipping");
            continue;
        }

        let period_start = report.declaration_date;
        let period_end = report.declaration_date;

        let decl = save_declaration(
            storage,
            &report.filename,
            &report.xml_content,
            DeclarationType::Ppo,
            period_start,
            period_end,
            &report.entries,
            &report.metadata(),
            output_dir,
        )?;

        info!(filename = %report.filename, "created PP-OPO declaration");
        created.push(decl);
    }

    Ok(IncomeOutcome::Created(created))
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
    let next_id = existing.len() + 1;
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
    fn test_income_period_no_last_date() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let end = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let opts = SyncOptions::default();

        let result = determine_income_period(&storage, end, &opts);
        assert!(result.is_some());
        let (start, pend) = result.unwrap();
        assert_eq!(pend, end);
        assert_eq!(start, end - Duration::days(DEFAULT_LOOKBACK_DAYS - 1));
    }

    #[test]
    fn test_income_period_with_last_date() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let last = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        storage.set_last_declaration_date(last).unwrap();

        let end = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let opts = SyncOptions::default();

        let result = determine_income_period(&storage, end, &opts);
        assert!(result.is_some());
        let (start, pend) = result.unwrap();
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 2, 16).unwrap());
        assert_eq!(pend, end);
    }

    #[test]
    fn test_income_period_last_date_equals_end() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let date = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        storage.set_last_declaration_date(date).unwrap();

        let opts = SyncOptions::default();
        let result = determine_income_period(&storage, date, &opts);
        assert!(result.is_none(), "start > end should yield None");
    }

    #[test]
    fn test_forced_lookback_overrides_start() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        let last = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        storage.set_last_declaration_date(last).unwrap();

        let end = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let opts = SyncOptions {
            force: false,
            forced_lookback_days: Some(90),
        };

        let result = determine_income_period(&storage, end, &opts);
        assert!(result.is_some());
        let (start, _) = result.unwrap();
        assert_eq!(start, end - Duration::days(89));
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
    fn run_sync_ibkr_error_propagates() {
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
        let result = run_sync(&storage, &nbs, &cfg, &cal, &opts, &ibkr);
        assert!(result.is_err());
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
