use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Duration, Local, NaiveDate};
use tracing::{debug, info, warn};

use crate::config;
use crate::holidays::HolidayCalendar;
use crate::ibkr_flex::{IBKRClient, parse_flex_report};
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

pub struct SyncResult {
    pub declarations_created: usize,
    pub gains_skipped: bool,
    pub income_skipped: bool,
    pub end_period: NaiveDate,
}

pub fn run_sync(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    options: &SyncOptions,
) -> Result<SyncResult> {
    validate_config(config)?;

    info!("fetching IBKR data…");
    let client = IBKRClient::new(&config.ibkr_token, &config.ibkr_query_id);
    let xml = client
        .fetch_latest_report()
        .context("failed to fetch IBKR Flex Query report")?;

    let report_date = Local::now().date_naive();
    storage.save_raw_report(&xml, report_date)?;

    let transactions = parse_flex_report(&xml)?;
    let (inserted, updated) = storage.save_transactions(&transactions)?;
    info!(inserted, updated, "saved transactions");

    let end_period = Local::now().date_naive() - Duration::days(1);

    let output_dir = config::get_effective_output_dir_path(config);
    std::fs::create_dir_all(&output_dir)?;

    let mut created = 0;
    let mut gains_skipped = false;
    let mut income_skipped = false;

    match generate_and_save_gains(
        storage,
        nbs,
        config,
        holidays,
        end_period,
        &output_dir,
        options,
    ) {
        Ok(n) => created += n,
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
        Ok(IncomeOutcome::Created(n)) => created += n,
        Ok(IncomeOutcome::NoIncome) => {
            debug!("no income in period, skipping");
            income_skipped = true;
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("no NBS exchange rate") {
                debug!(error = %e, "income report skipped");
                income_skipped = true;
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
        storage.set_last_declaration_date(end_period)?;
        debug!(%end_period, "updated last_declaration_date");
    }

    Ok(SyncResult {
        declarations_created: created,
        gains_skipped,
        income_skipped,
        end_period,
    })
}

fn validate_config(config: &UserConfig) -> Result<()> {
    let empty_fields: Vec<&str> = [
        ("ibkr_token", config.ibkr_token.as_str()),
        ("ibkr_query_id", config.ibkr_query_id.as_str()),
        ("personal_id", config.personal_id.as_str()),
        ("full_name", config.full_name.as_str()),
        ("address", config.address.as_str()),
        ("city_code", config.city_code.as_str()),
    ]
    .iter()
    .filter(|(_, v)| v.is_empty())
    .map(|(k, _)| *k)
    .collect();

    if !empty_fields.is_empty() {
        bail!(
            "missing required config fields: {}",
            empty_fields.join(", ")
        );
    }
    if config.phone == "0600000000" {
        bail!("phone is still the default placeholder — update your config");
    }
    if config.email == "email@example.com" {
        bail!("email is still the default placeholder — update your config");
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
) -> Result<usize> {
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
        return Ok(0);
    }

    save_declaration(
        storage,
        &report.filename,
        &report.xml_content,
        DeclarationType::Ppdg3r,
        report.period_start,
        report.period_end,
        &report.entries,
        &report.metadata(),
        output_dir,
    )?;

    info!(filename = %report.filename, "created PPDG-3R declaration");
    Ok(1)
}

enum IncomeOutcome {
    Created(usize),
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

    let mut created = 0;
    for report in &reports {
        if is_duplicate(storage, &report.filename, &DeclarationType::Ppo) {
            debug!(filename = %report.filename, "income declaration already exists, skipping");
            continue;
        }

        let period_start = report.declaration_date;
        let period_end = report.declaration_date;

        save_declaration(
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
        created += 1;
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
) -> Result<()> {
    let existing = storage.get_declarations(None, None);
    let next_id = existing.len() + 1;
    let id_str = next_id.to_string();
    let proper_filename = format!("{next_id:03}-{generator_filename}");

    let decl_path = storage.declarations_dir().join(&proper_filename);
    std::fs::write(&decl_path, xml_content)?;

    let output_path = output_dir.join(&proper_filename);
    std::fs::write(&output_path, xml_content)?;

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

    storage.save_declaration(&decl)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_validate_config_missing_fields() {
        let cfg = UserConfig::default();
        let result = validate_config(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_placeholder_phone() {
        let cfg = UserConfig {
            ibkr_token: "tok".into(),
            ibkr_query_id: "qid".into(),
            personal_id: "123".into(),
            full_name: "Test".into(),
            address: "Addr".into(),
            city_code: "223".into(),
            phone: "0600000000".into(),
            email: "test@test.com".into(),
            data_dir: None,
            output_folder: None,
        };
        let result = validate_config(&cfg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("phone"));
    }

    #[test]
    fn test_validate_config_ok() {
        let cfg = UserConfig {
            ibkr_token: "tok".into(),
            ibkr_query_id: "qid".into(),
            personal_id: "123".into(),
            full_name: "Test".into(),
            address: "Addr".into(),
            city_code: "223".into(),
            phone: "0641234567".into(),
            email: "test@test.com".into(),
            data_dir: None,
            output_folder: None,
        };
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_validate_config_placeholder_email() {
        let cfg = UserConfig {
            ibkr_token: "tok".into(),
            ibkr_query_id: "qid".into(),
            personal_id: "123".into(),
            full_name: "Test".into(),
            address: "Addr".into(),
            city_code: "223".into(),
            phone: "0641234567".into(),
            email: "email@example.com".into(),
            data_dir: None,
            output_folder: None,
        };
        let err = validate_config(&cfg).unwrap_err();
        assert!(err.to_string().contains("email"));
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
}
