use anyhow::Result;
use console::style;

use super::{StatusFilter, load_config_or_exit, make_storage, tables};
use ibkr_porez::list::{self, ListOptions};

#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn run(all: bool, status: Option<StatusFilter>, ids_only: bool) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);

    let options = ListOptions {
        show_all: all,
        status: status.map(|s| s.to_model()),
    };
    let declarations = list::list_declarations(&storage, &options);

    if ids_only {
        for d in &declarations {
            println!("{}", d.declaration_id);
        }
        return Ok(());
    }

    println!("{}", style("Declarations").bold());
    let table = tables::render_declarations_table(&declarations);
    println!("{table}");

    Ok(())
}
