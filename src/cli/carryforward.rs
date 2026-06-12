use anyhow::Result;
use chrono::{Datelike, Local};

use ibkr_porez::models::sort_oldest_origin_first;

use super::{load_config_or_exit, make_storage, output, tables};

#[allow(clippy::unnecessary_wraps)]
pub fn run() -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);

    let mut vintages = storage.get_carryforward_vintages();
    if vintages.is_empty() {
        output::info("No carryforward vintages recorded.");
        return Ok(());
    }
    sort_oldest_origin_first(&mut vintages);

    let current_year = Local::now().year();
    println!(
        "{}",
        tables::render_carryforward_table(&vintages, current_year)
    );
    Ok(())
}
