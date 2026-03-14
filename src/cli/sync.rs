use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use anyhow::Result;
use chrono::{Datelike, Local};

use super::{load_config_or_exit, make_nbs, make_storage, output, tables};
use ibkr_porez::config as app_config;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::{DeclarationType, IncomeDeclarationEntry, TaxReportEntry, UserConfig};
use ibkr_porez::openholiday::OpenHolidayClient;
use ibkr_porez::sync::{SyncOptions, run_sync};

#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn run(output_dir: Option<PathBuf>, lookback: Option<i64>) -> Result<()> {
    let mut cfg = load_config_or_exit();

    if let Some(ref out) = output_dir {
        cfg.output_folder = Some(out.display().to_string());
    }

    let storage = make_storage(&cfg);
    let cal = init_calendar_with_sync(&cfg);
    let nbs = make_nbs(&storage, &cal);

    let options = SyncOptions {
        force: false,
        forced_lookback_days: lookback,
    };

    let sp = output::spinner("Syncing data and creating declarations...");

    let result = match run_sync(&storage, &nbs, &cfg, &cal, &options) {
        Ok(r) => {
            sp.finish_and_clear();
            r
        }
        Err(e) => {
            sp.finish_and_clear();
            output::error(&format!("{e}"));
            return Ok(());
        }
    };

    if result.created_declarations.is_empty() {
        output::warning("No new declarations created.");
    } else {
        for decl in &result.created_declarations {
            output::success(&format!(
                "Created declaration {} ({})",
                decl.declaration_id,
                decl.display_type()
            ));

            if let Some(ref data) = decl.report_data {
                if decl.r#type == DeclarationType::Ppdg3r {
                    let entries: Vec<TaxReportEntry> = data
                        .iter()
                        .filter_map(|v| serde_json::from_value(v.clone()).ok())
                        .collect();
                    if !entries.is_empty() {
                        println!("\n  Declaration Data (Part 4)");
                        println!("{}", tables::render_gains_table(&entries));
                    }
                } else {
                    let entries: Vec<IncomeDeclarationEntry> = data
                        .iter()
                        .filter_map(|v| serde_json::from_value(v.clone()).ok())
                        .collect();
                    for entry in &entries {
                        tables::print_income_entry(entry);
                    }
                }
            }
        }
    }

    if let Some(ref err_msg) = result.income_error {
        output::error(&format!("Income report generation failed: {err_msg}"));
    }

    if result.gains_skipped {
        output::dim("  (gains report skipped — no taxable sales in period)");
    }
    if result.income_skipped {
        output::dim("  (income report skipped — no income in period)");
    }

    println!();
    output::dim("Use `ibkr-porez list` to see all declarations.");
    output::dim("Use `ibkr-porez show <ID>` for details.");
    output::dim("Use `ibkr-porez submit <ID>` to mark as submitted.");
    output::dim("Use `ibkr-porez pay <ID>` to mark as paid.");

    Ok(())
}

fn init_calendar_with_sync(cfg: &UserConfig) -> HolidayCalendar {
    let mut cal = HolidayCalendar::load_embedded();
    let data_dir = app_config::get_effective_data_dir_path(cfg);
    cal.merge_file(&data_dir);

    let current_year = Local::now().year();
    let mut years_to_fetch = Vec::new();
    if !cal.is_year_loaded(current_year) {
        years_to_fetch.push(current_year);
    }

    let threshold_day = next_year_fetch_threshold(current_year);
    let now = Local::now();
    if now.ordinal() >= threshold_day && !cal.is_year_loaded(current_year + 1) {
        years_to_fetch.push(current_year + 1);
    }

    if !years_to_fetch.is_empty() {
        let from = *years_to_fetch.iter().min().unwrap();
        let to = *years_to_fetch.iter().max().unwrap();
        let client = OpenHolidayClient::new();
        match client.fetch_years(from, to) {
            Ok(year_map) => {
                for (year, dates) in year_map {
                    cal.add_year(year, dates);
                    output::dim(&format!("Fetched holidays for {year}."));
                }
            }
            Err(e) => {
                output::warning(&format!("Failed to fetch holidays: {e}"));
            }
        }
        if let Err(e) = cal.save_overlay(&data_dir) {
            output::warning(&format!("Failed to save holiday overlay: {e}"));
        }
    }

    cal
}

fn next_year_fetch_threshold(year: i32) -> u32 {
    let mut hasher = DefaultHasher::new();
    year.hash(&mut hasher);
    let h = hasher.finish();
    let offset = (h % 6) as u32;
    349 + offset
}
