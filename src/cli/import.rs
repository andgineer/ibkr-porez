use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

use anyhow::{Result, bail};

use super::{LibImportType, init_calendar, load_config_or_exit, make_nbs, make_storage, output};
use ibkr_porez::import::{self, FileType};

#[allow(clippy::needless_pass_by_value)]
pub fn run(file_path: Option<PathBuf>, import_type: LibImportType) -> Result<()> {
    let cfg = load_config_or_exit();
    let storage = make_storage(&cfg);
    let cal = init_calendar(&cfg);
    let nbs = make_nbs(&storage, &cal);

    let ft = match import_type {
        LibImportType::Auto => FileType::Auto,
        LibImportType::Csv => FileType::Csv,
        LibImportType::Flex => FileType::Flex,
    };

    let source = resolve_input(file_path)?;
    let label = match &source {
        InputSource::Stdin => "stdin",
        InputSource::File(p) => p.to_str().unwrap_or("file"),
    };
    output::info(&format!("Importing from {label}..."));
    let sp = output::spinner("Processing import...");

    let result = match &source {
        InputSource::Stdin => {
            let mut buf = Vec::new();
            io::stdin().read_to_end(&mut buf)?;
            let cursor = io::Cursor::new(buf);
            import::import_from_reader(&storage, &nbs, cursor, ft, None)
        }
        InputSource::File(path) => import::import_from_file(&storage, &nbs, path, ft),
    };

    sp.finish_and_clear();

    match result {
        Ok(r) => {
            if r.transaction_count == 0 {
                output::warning("No valid transactions found in file.");
            } else {
                output::success(&format!(
                    "Parsed {} transactions. ({} new, {} updated)",
                    r.transaction_count, r.inserted, r.updated,
                ));
                output::bold_success("Import Complete!");
            }
        }
        Err(e) => {
            output::error(&format!("{e}"));
        }
    }
    Ok(())
}

enum InputSource {
    Stdin,
    File(PathBuf),
}

fn resolve_input(file_path: Option<PathBuf>) -> Result<InputSource> {
    match file_path {
        None => {
            if io::stdin().is_terminal() {
                bail!(
                    "no file specified and stdin is a terminal; provide a file path or pipe data"
                );
            }
            Ok(InputSource::Stdin)
        }
        Some(p) if p.to_str() == Some("-") => Ok(InputSource::Stdin),
        Some(p) => {
            if p.exists() {
                Ok(InputSource::File(p))
            } else if !io::stdin().is_terminal() {
                Ok(InputSource::Stdin)
            } else {
                bail!("file not found: {}", p.display());
            }
        }
    }
}
