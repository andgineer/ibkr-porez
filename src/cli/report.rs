use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{Datelike, NaiveDate};

use super::{
    LibReportType, init_calendar, load_config_or_exit, make_nbs, make_storage, output,
    resolve_gains_period, resolve_income_period, tables,
};
use ibkr_porez::config as app_config;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::UserConfig;
use ibkr_porez::nbs::NBSClient;
use ibkr_porez::report_gains::{compute_carryforward_application, generate_gains_report};
use ibkr_porez::report_income::generate_income_reports;
use ibkr_porez::storage::Storage;

#[allow(clippy::needless_pass_by_value)]
pub fn run(
    report_type: LibReportType,
    half: Option<String>,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
    force: bool,
    output_dir: Option<PathBuf>,
) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let mut cal = init_calendar(&cfg);

    if force && matches!(report_type, LibReportType::Income) {
        cal.set_fallback(true);
    }

    let nbs = make_nbs(&storage, &cal);

    let dest_dir = output_dir.unwrap_or_else(|| app_config::get_effective_output_dir_path(&cfg));
    std::fs::create_dir_all(&dest_dir)?;

    match report_type {
        LibReportType::Gains => run_gains(
            &storage,
            &nbs,
            &cfg,
            &cal,
            half.as_deref(),
            start,
            end,
            &dest_dir,
        ),
        LibReportType::Income => run_income(
            &storage,
            &nbs,
            &cfg,
            &cal,
            half.as_deref(),
            start,
            end,
            force,
            &dest_dir,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_gains(
    storage: &Storage,
    nbs: &NBSClient,
    cfg: &UserConfig,
    cal: &HolidayCalendar,
    half: Option<&str>,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
    dest_dir: &Path,
) -> Result<()> {
    let (period_start, period_end) = resolve_gains_period(half, start, end)?;

    output::info(&format!(
        "Generating PPDG-3R Report for {} to {}",
        period_start.format("%Y-%m-%d"),
        period_end.format("%Y-%m-%d"),
    ));

    let report =
        match generate_gains_report(storage, nbs, cfg, cal, period_start, period_end, false) {
            Ok(r) => r,
            Err(e) => {
                output::error(&format!("{e:#}"));
                return Ok(());
            }
        };

    let dest = dest_dir.join(&report.filename);
    std::fs::write(&dest, &report.xml_content)?;

    println!("\n  Declaration Data (Part 4)");
    println!("{}", tables::render_gains_table(&report.entries));
    output::bold(&format!("Total Entries: {}", report.entries.len()));

    let cf_app =
        compute_carryforward_application(storage, report.tax_base(), report.period_end.year());
    println!();
    output::bold(&format!(
        "Calculated tax base:  {}",
        output::format_thousands_2f(report.tax_base())
    ));
    output::info(&format!(
        "Carryforward applied: {}",
        output::format_thousands_2f(cf_app.carryforward_used_rsd)
    ));
    output::bold(&format!(
        "Adjusted tax base:    {}",
        output::format_thousands_2f(cf_app.adjusted_tax_base_rsd)
    ));
    output::bold(&format!(
        "Estimated tax:        {}",
        output::format_thousands_2f(cf_app.estimated_tax_rsd)
    ));
    output::info(&format!(
        "Closing carryforward: {}",
        output::format_thousands_2f(cf_app.closing_carryforward_rsd)
    ));

    output::success(&format!("Report written to {}", dest.display()));
    output::attention(
        "ATTENTION: Step 8 requires uploading IBKR Activity Report PDF. \
         Download it from IBKR Account Management.",
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_income(
    storage: &Storage,
    nbs: &NBSClient,
    cfg: &UserConfig,
    cal: &HolidayCalendar,
    half: Option<&str>,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
    force: bool,
    dest_dir: &Path,
) -> Result<()> {
    let (period_start, period_end) = resolve_income_period(half, start, end)?;

    output::info(&format!(
        "Generating PP OPO Report for {} to {}",
        period_start.format("%Y-%m-%d"),
        period_end.format("%Y-%m-%d"),
    ));

    if force {
        output::warning(
            "WARNING: --force flag is set. Missing exchange rates will use nearest cached value.",
        );
    }

    let reports =
        match generate_income_reports(storage, nbs, cfg, cal, period_start, period_end, force) {
            Ok(r) => r,
            Err(e) => {
                output::error(&format!("{e:#}"));
                return Ok(());
            }
        };

    if reports.is_empty() {
        output::warning(&format!(
            "No income found in period {} to {}.",
            period_start.format("%Y-%m-%d"),
            period_end.format("%Y-%m-%d"),
        ));
        return Ok(());
    }

    for report in &reports {
        let dest = dest_dir.join(&report.filename);
        std::fs::write(&dest, &report.xml_content)?;
        output::success(&format!(
            "Report written to {} ({} entries)",
            dest.display(),
            report.entries.len(),
        ));
        for entry in &report.entries {
            tables::print_income_entry(entry);
        }
    }

    Ok(())
}
