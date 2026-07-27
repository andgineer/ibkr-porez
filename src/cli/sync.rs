use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Result, bail};

use super::{init_calendar_with_sync, load_config_or_exit, make_nbs, make_storage, output, tables};
use ibkr_porez::ibkr_flex::IBKRClient;
use ibkr_porez::models::{Declaration, DeclarationType, IncomeDeclarationEntry, TaxReportEntry};
use ibkr_porez::storage::Storage;
use ibkr_porez::sync::SyncResult;
use ibkr_porez::sync::{SyncOptions, run_sync, run_sync_from_file, run_sync_from_xml};

#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn run(
    output_dir: Option<PathBuf>,
    lookback: Option<i64>,
    file: Option<PathBuf>,
) -> Result<()> {
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

    let result = if let Some(ref path) = file {
        let sp = output::spinner("Importing from file and creating declarations...");
        let sync_result = if path.to_str() == Some("-") {
            if io::stdin().is_terminal() {
                bail!("--file - requires piped input (stdin is a terminal)");
            }
            let mut xml = String::new();
            io::stdin().read_to_string(&mut xml)?;
            run_sync_from_xml(&xml, &storage, &nbs, &cfg, &cal, &options)
        } else {
            run_sync_from_file(path, &storage, &nbs, &cfg, &cal, &options)
        };
        match sync_result {
            Ok(r) => {
                sp.finish_and_clear();
                r
            }
            Err(e) => {
                sp.finish_and_clear();
                output::error(&format!("{e:#}"));
                return Ok(());
            }
        }
    } else {
        let ibkr = IBKRClient::new(&cfg.ibkr_token, &cfg.ibkr_query_id);
        let sp = output::spinner("Syncing data and creating declarations...");
        match run_sync(&storage, &nbs, &cfg, &cal, &options, &ibkr) {
            Ok(r) => {
                sp.finish_and_clear();
                r
            }
            Err(e) => {
                sp.finish_and_clear();
                output::error(&format!("{e:#}"));
                return Ok(());
            }
        }
    };

    print_sync_result(&result, &storage);
    Ok(())
}

/// An amendment is printed like any other created declaration; the hint carries
/// the two things that locate the original in the ePorezi table, where every
/// other column is uninformative.
fn print_amendment_hint(decl: &Declaration, storage: &Storage) {
    let Some(amends_id) = decl.metadata.get("amends").and_then(|v| v.as_str()) else {
        return;
    };
    let symbol = decl
        .metadata
        .get("symbol")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let meta = |d: &Declaration, key: &str| {
        d.metadata
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string()
    };

    let original = storage.get_declaration(amends_id);
    let number = original
        .as_ref()
        .and_then(|d| {
            d.metadata
                .get(ibkr_porez::declaration_manager::PURS_NUMBER_KEY)
        })
        .and_then(|v| v.as_str())
        .map_or_else(
            || "number not recorded — find it by date".to_string(),
            |n| format!("number {n}"),
        );
    let credit_before = original
        .as_ref()
        .map_or_else(|| "?".to_string(), |d| meta(d, "foreign_tax_paid_rsd"));

    output::info(&format!(
        "Amended PP-OPO: датум остваривања прихода {}, {symbol}",
        decl.period_start.format("%d.%m.%Y"),
    ));
    output::dim(&format!("  original: declaration {amends_id}, {number}"));
    output::dim(&format!(
        "  credit {credit_before} → {} RSD, now due {} RSD",
        meta(decl, "foreign_tax_paid_rsd"),
        meta(decl, "tax_due_rsd"),
    ));
}

fn print_sync_result(result: &SyncResult, storage: &Storage) {
    if let Some(ref err_msg) = result.fetch_error {
        output::warning(&format!(
            "IBKR fetch failed ({err_msg}); generated declarations from stored transactions. Re-run `sync` later for fresh data."
        ));
    }

    if result.created_declarations.is_empty() {
        output::warning("No new declarations created.");
    } else {
        for decl in &result.created_declarations {
            output::success(&format!(
                "Created declaration {} ({})",
                decl.declaration_id,
                decl.display_type()
            ));
            print_amendment_hint(decl, storage);

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

    for notice in &result.income_notices {
        output::dim(&format!("  {notice}"));
    }

    if result.gains_skipped {
        output::dim("  (gains report skipped — no taxable sales in period)");
    }
    if result.income_skipped {
        output::dim("  (income report skipped — no undeclared income in period)");
    }

    println!();
    output::dim("Use `ibkr-porez list` to see all declarations.");
    output::dim("Use `ibkr-porez show <ID>` for details.");
    output::dim("Use `ibkr-porez submit <ID> [<ID> ...]` to mark as submitted.");
    output::dim("Use `ibkr-porez pay <ID> [<ID> ...]` to mark as paid.");
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::NaiveDate;
    use ibkr_porez::models::DeclarationStatus;

    fn tmp_storage() -> (tempfile::TempDir, Storage) {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage = Storage::with_dir(tmp.path());
        (tmp, storage)
    }

    fn make_sync_result(
        decls: Vec<Declaration>,
        gains_skipped: bool,
        income_skipped: bool,
        income_notices: Vec<String>,
    ) -> SyncResult {
        SyncResult {
            created_declarations: decls,
            gains_skipped,
            income_skipped,
            income_notices,
            fetch_error: None,
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
        let result = make_sync_result(vec![], false, false, Vec::new());
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
    }

    #[test]
    fn print_gains_skipped() {
        let result = make_sync_result(vec![], true, false, Vec::new());
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
    }

    #[test]
    fn print_income_skipped() {
        let result = make_sync_result(vec![], false, true, Vec::new());
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
    }

    #[test]
    fn print_both_skipped() {
        let result = make_sync_result(vec![], true, true, Vec::new());
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
    }

    #[test]
    fn print_income_notices() {
        let result = make_sync_result(
            vec![],
            false,
            false,
            vec!["VOO 2026-03-01: no NBS exchange rate".into()],
        );
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
    }

    #[test]
    fn print_created_ppdg3r_declaration() {
        let decl = sample_declaration("gains-1", DeclarationType::Ppdg3r);
        let result = make_sync_result(vec![decl], false, false, Vec::new());
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
    }

    #[test]
    fn print_created_ppo_declaration() {
        let decl = sample_declaration("income-1", DeclarationType::Ppo);
        let result = make_sync_result(vec![decl], false, false, Vec::new());
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
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
        let result = make_sync_result(vec![decl], false, false, Vec::new());
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
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
        let result = make_sync_result(vec![decl], false, false, Vec::new());
        let (_tmp, storage) = tmp_storage();
        print_sync_result(&result, &storage);
    }
}
