use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::ibkr_csv;
use crate::models::Transaction;
use crate::nbs::NBSClient;
use crate::storage::Storage;

pub struct ImportResult {
    pub inserted: usize,
    pub updated: usize,
    pub transaction_count: usize,
}

pub fn import_from_reader<R: Read>(
    storage: &Storage,
    nbs: &NBSClient,
    reader: R,
) -> Result<ImportResult> {
    let mut buf = Vec::new();
    BufReader::new(reader)
        .read_to_end(&mut buf)
        .context("failed to read input")?;

    let cursor = std::io::Cursor::new(buf);
    let transactions =
        ibkr_csv::parse_csv_activity(BufReader::new(cursor)).context("failed to parse CSV")?;

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

pub fn import_from_file(storage: &Storage, nbs: &NBSClient, path: &Path) -> Result<ImportResult> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open file: {}", path.display()))?;
    import_from_reader(storage, nbs, file)
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
