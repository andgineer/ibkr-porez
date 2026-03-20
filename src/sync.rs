use std::path::Path;

use anyhow::{Result, bail};
use chrono::{Datelike, Duration, Local, NaiveDate};
use tracing::{debug, info, warn};

use crate::config;
use crate::fetch;
use crate::holidays::HolidayCalendar;
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
) -> Result<SyncResult> {
    validate_config_or_bail(config)?;

    let fetch_result = fetch::fetch_and_import(storage, nbs, config)?;
    info!(
        inserted = fetch_result.inserted,
        updated = fetch_result.updated,
        total = fetch_result.transactions.len(),
        "fetch complete"
    );

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
        storage.set_last_declaration_date(end_period)?;
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

    let decl = save_declaration(
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
    Ok(decl)
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
