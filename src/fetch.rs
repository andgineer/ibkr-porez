use anyhow::{Context, Result};
use chrono::Local;
use tracing::info;

use crate::ibkr_flex::{IBKRClient, parse_flex_report};
use crate::models::{Transaction, UserConfig};
use crate::nbs::NBSClient;
use crate::storage::Storage;

pub struct FetchResult {
    pub transactions: Vec<Transaction>,
    pub inserted: usize,
    pub updated: usize,
}

/// Download the latest IBKR Flex report, parse it, save transactions, and
/// pre-fetch NBS exchange rates. This is the data-retrieval subset of `run_sync`.
pub fn fetch_and_import(
    storage: &Storage,
    nbs: &NBSClient,
    config: &UserConfig,
) -> Result<FetchResult> {
    validate_ibkr_config(config)?;

    info!("fetching IBKR data…");
    let client = IBKRClient::new(&config.ibkr_token, &config.ibkr_query_id);
    let xml = client
        .fetch_latest_report()
        .context("failed to fetch IBKR Flex Query report")?;

    let report_date = Local::now().date_naive();
    storage.save_raw_report(&xml, report_date)?;

    let transactions = parse_flex_report(&xml)?;
    let (inserted, updated) = storage.save_transactions(&transactions)?;
    info!(inserted, updated, "saved transactions");

    prefetch_rates(storage, nbs, &transactions);

    Ok(FetchResult {
        transactions,
        inserted,
        updated,
    })
}

pub fn validate_ibkr_config(config: &UserConfig) -> Result<()> {
    if config.ibkr_token.is_empty() || config.ibkr_query_id.is_empty() {
        anyhow::bail!(
            "Missing IBKR configuration. Run `ibkr-porez config` first \
             to set your IBKR token and query ID."
        );
    }
    Ok(())
}

fn prefetch_rates(storage: &Storage, nbs: &NBSClient, transactions: &[Transaction]) {
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

    let rates = storage.load_rates();
    info!(cached_rates = rates.len(), "rate prefetch complete");
}
