use anyhow::Result;

use super::{init_calendar, load_config_or_exit, make_nbs, make_storage, output};

#[allow(clippy::unnecessary_wraps)]
pub fn run() -> Result<()> {
    let cfg = load_config_or_exit();

    if let Err(e) = ibkr_porez::fetch::validate_ibkr_config(&cfg) {
        output::error(&format!("{e}"));
        return Ok(());
    }

    let storage = make_storage(&cfg);
    let cal = init_calendar(&cfg);
    let nbs = make_nbs(&storage, &cal);

    output::info("Fetching full report...");
    let sp = output::spinner("Fetching data from IBKR...");

    let result = match ibkr_porez::fetch::fetch_and_import(&storage, &nbs, &cfg) {
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

    output::success(&format!(
        "Fetched {} transactions. ({} new, {} updated)",
        result.transactions.len(),
        result.inserted,
        result.updated,
    ));
    output::bold_success("Sync Complete!");
    Ok(())
}
