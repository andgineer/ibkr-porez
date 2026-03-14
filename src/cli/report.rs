use std::path::PathBuf;

use anyhow::Result;
use chrono::NaiveDate;

use super::{
    LibReportType, init_calendar, load_config_or_exit, make_nbs, make_storage, output,
    resolve_gains_period, resolve_income_period, tables,
};
use ibkr_porez::config as app_config;
use ibkr_porez::report_gains::generate_gains_report;
use ibkr_porez::report_income::generate_income_reports;

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
        LibReportType::Gains => {
            let (period_start, period_end) = resolve_gains_period(half.as_deref(), start, end)?;

            output::info(&format!(
                "Generating PPDG-3R Report for {} to {}",
                period_start.format("%Y-%m-%d"),
                period_end.format("%Y-%m-%d"),
            ));

            let report = match generate_gains_report(
                &storage,
                &nbs,
                &cfg,
                &cal,
                period_start,
                period_end,
                false,
            ) {
                Ok(r) => r,
                Err(e) => {
                    output::error(&format!("{e}"));
                    return Ok(());
                }
            };

            let dest = dest_dir.join(&report.filename);
            std::fs::write(&dest, &report.xml_content)?;

            println!("\n  Declaration Data (Part 4)");
            println!("{}", tables::render_gains_table(&report.entries));
            output::bold(&format!("Total Entries: {}", report.entries.len()));
            output::success(&format!("Report written to {}", dest.display()));
            output::attention(
                "ATTENTION: Step 8 requires uploading IBKR Activity Report PDF. \
                 Download it from IBKR Account Management.",
            );
        }
        LibReportType::Income => {
            let (period_start, period_end) = resolve_income_period(half.as_deref(), start, end)?;

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

            let reports = match generate_income_reports(
                &storage,
                &nbs,
                &cfg,
                &cal,
                period_start,
                period_end,
                force,
            ) {
                Ok(r) => r,
                Err(e) => {
                    output::error(&format!("{e}"));
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
        }
    }

    Ok(())
}
