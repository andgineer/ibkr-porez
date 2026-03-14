use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use anyhow::Result;
use chrono::NaiveDate;

use super::{load_config_or_exit, make_storage, output};
use ibkr_porez::config as app_config;

pub fn run(date: NaiveDate, output_path: Option<String>) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);

    let Some(xml) = storage.restore_report(date) else {
        output::error(&format!("No flex query report found for {date}."));
        return Ok(());
    };

    let dest = resolve_output(output_path, date, &cfg)?;

    match dest {
        OutputDest::Stdout => {
            io::stdout().write_all(xml.as_bytes())?;
        }
        OutputDest::File(path) => {
            std::fs::write(&path, &xml)?;
            let abs = std::fs::canonicalize(&path).unwrap_or(path);
            output::success(&format!("Exported flex query saved to: {}", abs.display()));
        }
    }

    Ok(())
}

enum OutputDest {
    Stdout,
    File(PathBuf),
}

fn resolve_output(
    output_path: Option<String>,
    date: NaiveDate,
    cfg: &ibkr_porez::models::UserConfig,
) -> Result<OutputDest> {
    match output_path {
        Some(ref s) if s == "-" => Ok(OutputDest::Stdout),
        Some(s) => {
            let mut path = PathBuf::from(&s);
            if path
                .extension()
                .and_then(|e| e.to_str())
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("xml"))
            {
                path.set_extension("xml");
            }
            Ok(OutputDest::File(path))
        }
        None => {
            if !io::stdout().is_terminal() {
                return Ok(OutputDest::Stdout);
            }
            let output_dir = app_config::get_effective_output_dir_path(cfg);
            std::fs::create_dir_all(&output_dir)?;
            let filename = format!("flex_query_{}.xml", date.format("%Y%m%d"));
            Ok(OutputDest::File(output_dir.join(filename)))
        }
    }
}
