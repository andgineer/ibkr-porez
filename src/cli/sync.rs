use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use anyhow::Result;
use chrono::{Datelike, Local};

use super::{load_config_or_exit, make_nbs, make_storage, output, tables};
use ibkr_porez::config as app_config;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::ibkr_flex::IBKRClient;
use ibkr_porez::models::{DeclarationType, IncomeDeclarationEntry, TaxReportEntry, UserConfig};
use ibkr_porez::openholiday::OpenHolidayClient;
use ibkr_porez::sync::SyncResult;
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
    let ibkr = IBKRClient::new(&cfg.ibkr_token, &cfg.ibkr_query_id);

    let options = SyncOptions {
        force: false,
        forced_lookback_days: lookback,
    };

    let sp = output::spinner("Syncing data and creating declarations...");

    let result = match run_sync(&storage, &nbs, &cfg, &cal, &options, &ibkr) {
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

    print_sync_result(&result);
    Ok(())
}

fn print_sync_result(result: &SyncResult) {
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
                        .filter_map(|v| serde_json::from_value::<TaxReportEntry>(v.clone()).ok())
                        .collect();
                    if !entries.is_empty() {
                        println!("\n  Declaration Data (Part 4)");
                        println!("{}", tables::render_gains_table(&entries));
                    }
                } else {
                    let entries: Vec<IncomeDeclarationEntry> = data
                        .iter()
                        .filter_map(|v| {
                            serde_json::from_value::<IncomeDeclarationEntry>(v.clone()).ok()
                        })
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
    output::dim("Use `ibkr-porez submit <ID> [<ID> ...]` to mark as submitted.");
    output::dim("Use `ibkr-porez pay <ID> [<ID> ...]` to mark as paid.");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_within_expected_range() {
        for year in 2020..=2035 {
            let t = next_year_fetch_threshold(year);
            assert!(
                (349..=354).contains(&t),
                "threshold {t} for year {year} out of range 349..=354"
            );
        }
    }

    #[test]
    fn threshold_deterministic() {
        let a = next_year_fetch_threshold(2026);
        let b = next_year_fetch_threshold(2026);
        assert_eq!(a, b);
    }

    #[test]
    fn threshold_varies_across_years() {
        let values: std::collections::HashSet<u32> =
            (2020..=2035).map(next_year_fetch_threshold).collect();
        assert!(
            values.len() > 1,
            "threshold should vary across years, got single value"
        );
    }

    use chrono::NaiveDate;
    use ibkr_porez::models::{Declaration, DeclarationStatus};

    fn make_sync_result(
        decls: Vec<Declaration>,
        gains_skipped: bool,
        income_skipped: bool,
        income_error: Option<String>,
    ) -> SyncResult {
        SyncResult {
            created_declarations: decls,
            gains_skipped,
            income_skipped,
            income_error,
            end_period: NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
        }
    }

    fn sample_declaration(id: &str, dtype: DeclarationType) -> Declaration {
        Declaration {
            declaration_id: id.into(),
            r#type: dtype,
            status: DeclarationStatus::Draft,
            period_start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            period_end: NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
            created_at: chrono::Local::now().naive_local(),
            submitted_at: None,
            paid_at: None,
            file_path: None,
            xml_content: Some("<xml/>".into()),
            report_data: None,
            metadata: indexmap::IndexMap::new(),
            attached_files: indexmap::IndexMap::new(),
        }
    }

    #[test]
    fn print_no_declarations() {
        let result = make_sync_result(vec![], false, false, None);
        print_sync_result(&result);
    }

    #[test]
    fn print_gains_skipped() {
        let result = make_sync_result(vec![], true, false, None);
        print_sync_result(&result);
    }

    #[test]
    fn print_income_skipped() {
        let result = make_sync_result(vec![], false, true, None);
        print_sync_result(&result);
    }

    #[test]
    fn print_both_skipped() {
        let result = make_sync_result(vec![], true, true, None);
        print_sync_result(&result);
    }

    #[test]
    fn print_income_error() {
        let result = make_sync_result(vec![], false, false, Some("NBS rate missing".into()));
        print_sync_result(&result);
    }

    #[test]
    fn print_created_ppdg3r_declaration() {
        let decl = sample_declaration("gains-1", DeclarationType::Ppdg3r);
        let result = make_sync_result(vec![decl], false, false, None);
        print_sync_result(&result);
    }

    #[test]
    fn print_created_ppo_declaration() {
        let decl = sample_declaration("income-1", DeclarationType::Ppo);
        let result = make_sync_result(vec![decl], false, false, None);
        print_sync_result(&result);
    }

    #[test]
    fn print_ppdg3r_with_report_data() {
        let mut decl = sample_declaration("gains-2", DeclarationType::Ppdg3r);
        let entry = TaxReportEntry {
            ticker: "AAPL".into(),
            quantity: rust_decimal::Decimal::new(10, 0),
            sale_date: NaiveDate::from_ymd_opt(2025, 3, 15).unwrap(),
            sale_price: rust_decimal::Decimal::new(174, 0),
            sale_exchange_rate: rust_decimal::Decimal::new(108, 0),
            sale_value_rsd: rust_decimal::Decimal::new(18792, 0),
            purchase_date: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            purchase_price: rust_decimal::Decimal::new(150, 0),
            purchase_exchange_rate: rust_decimal::Decimal::new(108, 0),
            purchase_value_rsd: rust_decimal::Decimal::new(16200, 0),
            capital_gain_rsd: rust_decimal::Decimal::new(2592, 0),
            is_tax_exempt: false,
        };
        decl.report_data = Some(vec![serde_json::to_value(entry).unwrap()]);
        let result = make_sync_result(vec![decl], false, false, None);
        print_sync_result(&result);
    }

    #[test]
    fn print_ppo_with_report_data() {
        let mut decl = sample_declaration("income-2", DeclarationType::Ppo);
        let entry = IncomeDeclarationEntry {
            date: NaiveDate::from_ymd_opt(2025, 3, 10).unwrap(),
            symbol_or_currency: Some("AAPL".into()),
            sifra_vrste_prihoda: "1070".into(),
            bruto_prihod: rust_decimal::Decimal::new(10800, 2),
            osnovica_za_porez: rust_decimal::Decimal::new(10800, 2),
            obracunati_porez: rust_decimal::Decimal::new(1620, 2),
            porez_placen_drugoj_drzavi: rust_decimal::Decimal::new(1620, 2),
            porez_za_uplatu: rust_decimal::Decimal::ZERO,
        };
        decl.report_data = Some(vec![serde_json::to_value(entry).unwrap()]);
        let result = make_sync_result(vec![decl], false, false, None);
        print_sync_result(&result);
    }

    #[test]
    fn init_calendar_returns_loaded_calendar() {
        let cfg = UserConfig {
            data_dir: Some(
                tempfile::TempDir::new()
                    .unwrap()
                    .path()
                    .display()
                    .to_string(),
            ),
            ..UserConfig::default()
        };
        let cal = init_calendar_with_sync(&cfg);
        let current_year = Local::now().year();
        assert!(
            cal.is_year_loaded(current_year),
            "calendar should have current year after sync"
        );
    }
}
