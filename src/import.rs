use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::ibkr_csv;
use crate::ibkr_flex;
use crate::models::Transaction;
use crate::nbs::NBSClient;
use crate::storage::Storage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Auto,
    Csv,
    Flex,
}

pub struct ImportResult {
    pub inserted: usize,
    pub updated: usize,
    pub transaction_count: usize,
}

/// Import transactions from a reader, auto-detecting format if needed.
pub fn import_from_reader<R: Read>(
    storage: &Storage,
    nbs: &NBSClient,
    reader: R,
    file_type: FileType,
    filename_hint: Option<&str>,
) -> Result<ImportResult> {
    let mut buf = Vec::new();
    BufReader::new(reader)
        .read_to_end(&mut buf)
        .context("failed to read input")?;

    let detected = if file_type == FileType::Auto {
        detect_type(filename_hint, &buf)
    } else {
        file_type
    };

    let content = String::from_utf8_lossy(&buf);

    let transactions = match detected {
        FileType::Csv => {
            let cursor = std::io::Cursor::new(content.as_bytes());
            ibkr_csv::parse_csv_activity(BufReader::new(cursor)).context("failed to parse CSV")?
        }
        FileType::Flex | FileType::Auto => {
            ibkr_flex::parse_flex_report(&content).context("failed to parse Flex XML")?
        }
    };

    let transaction_count = transactions.len();
    let (inserted, updated) = storage.save_transactions(&transactions)?;
    info!(
        inserted,
        updated,
        total = transaction_count,
        "imported transactions"
    );

    prefetch_rates(nbs, &transactions);

    Ok(ImportResult {
        inserted,
        updated,
        transaction_count,
    })
}

/// Import from a file path.
pub fn import_from_file(
    storage: &Storage,
    nbs: &NBSClient,
    path: &Path,
    file_type: FileType,
) -> Result<ImportResult> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open file: {}", path.display()))?;
    let filename = path.file_name().and_then(|n| n.to_str());
    import_from_reader(storage, nbs, file, file_type, filename)
}

fn detect_type(filename_hint: Option<&str>, content: &[u8]) -> FileType {
    if let Some(name) = filename_hint {
        let path = std::path::Path::new(name);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if ext_lower == "csv" {
                return FileType::Csv;
            }
            if ext_lower == "xml" || ext_lower == "zip" {
                return FileType::Flex;
            }
        }
    }

    let peek = &content[..content.len().min(1000)];
    let peek_str = String::from_utf8_lossy(peek);
    if peek_str.contains("<?xml") || peek_str.contains("<FlexQueryResponse") {
        return FileType::Flex;
    }

    FileType::Csv
}

fn prefetch_rates(nbs: &NBSClient, transactions: &[Transaction]) {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for txn in transactions {
        let key = (txn.date, txn.currency.clone());
        if seen.insert(key)
            && let Err(e) = nbs.get_rate(txn.date, &txn.currency)
        {
            tracing::debug!(date = %txn.date, error = %e, "rate prefetch failed (non-fatal)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_csv_by_extension() {
        assert_eq!(detect_type(Some("data.csv"), b"anything"), FileType::Csv);
        assert_eq!(detect_type(Some("DATA.CSV"), b"anything"), FileType::Csv);
    }

    #[test]
    fn detect_xml_by_extension() {
        assert_eq!(detect_type(Some("report.xml"), b"anything"), FileType::Flex);
        assert_eq!(detect_type(Some("report.XML"), b"anything"), FileType::Flex);
    }

    #[test]
    fn detect_zip_by_extension() {
        assert_eq!(detect_type(Some("report.zip"), b"anything"), FileType::Flex);
    }

    #[test]
    fn detect_flex_by_xml_content() {
        assert_eq!(
            detect_type(None, b"<?xml version=\"1.0\"?>"),
            FileType::Flex
        );
        assert_eq!(
            detect_type(None, b"<FlexQueryResponse queryName="),
            FileType::Flex
        );
    }

    #[test]
    fn detect_csv_as_fallback() {
        assert_eq!(
            detect_type(None, b"date,symbol,amount\n2024-01-01,AAPL,100"),
            FileType::Csv
        );
    }

    #[test]
    fn detect_prefers_extension_over_content() {
        assert_eq!(
            detect_type(Some("data.csv"), b"<?xml version=\"1.0\"?>"),
            FileType::Csv
        );
    }
}
