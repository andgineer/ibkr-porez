use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::NaiveDate;
use indexmap::IndexMap;
use rust_decimal::Decimal;

use crate::config;
use crate::models::{
    Currency, Declaration, DeclarationStatus, DeclarationType, DeclarationsFile, ExchangeRate,
    Transaction, TransactionKey, UserConfig,
};

const RATES_FILENAME: &str = "rates.json";
const DECLARATIONS_FILENAME: &str = "declarations.json";
const TRANSACTIONS_FILENAME: &str = "transactions.json";
const DECLARATIONS_DIR: &str = "declarations";
const FLEX_QUERIES_DIR: &str = "flex-queries";
const APP_NAME: &str = "ibkr-porez";
const DATA_SUBDIR: &str = "ibkr-porez-data";

pub struct Storage {
    data_dir: PathBuf,
    transactions_file: PathBuf,
    rates_file: PathBuf,
    declarations_file: PathBuf,
    declarations_dir: PathBuf,
    flex_queries_dir: PathBuf,
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage {
    #[must_use]
    pub fn new() -> Self {
        let cfg = config::load_config();
        Self::with_config(&cfg)
    }

    #[must_use]
    pub fn with_config(cfg: &UserConfig) -> Self {
        let app_data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME);
        let data_dir = match &cfg.data_dir {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => app_data_dir.join(DATA_SUBDIR),
        };

        let s = Self {
            transactions_file: data_dir.join(TRANSACTIONS_FILENAME),
            rates_file: data_dir.join(RATES_FILENAME),
            declarations_file: data_dir.join(DECLARATIONS_FILENAME),
            declarations_dir: data_dir.join(DECLARATIONS_DIR),
            flex_queries_dir: app_data_dir.join(FLEX_QUERIES_DIR),
            data_dir,
        };
        s.ensure_dirs();
        s
    }

    /// Build a `Storage` rooted at the given directory (useful for tests).
    #[must_use]
    pub fn with_dir(dir: &Path) -> Self {
        let s = Self {
            data_dir: dir.to_path_buf(),
            transactions_file: dir.join(TRANSACTIONS_FILENAME),
            rates_file: dir.join(RATES_FILENAME),
            declarations_file: dir.join(DECLARATIONS_FILENAME),
            declarations_dir: dir.join(DECLARATIONS_DIR),
            flex_queries_dir: dir.join(FLEX_QUERIES_DIR),
        };
        s.ensure_dirs();
        s
    }

    fn ensure_dirs(&self) {
        let _ = std::fs::create_dir_all(&self.data_dir);
        let _ = std::fs::create_dir_all(&self.declarations_dir);
        let _ = std::fs::create_dir_all(&self.flex_queries_dir);
    }

    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Return a diagnostic hint when a storage I/O operation fails.
    /// Helps the user understand that the error originates from the data
    /// directory (which may be on a cloud-synced or network filesystem).
    #[must_use]
    pub fn io_error_hint(&self) -> String {
        format!(
            "Failed to write to data directory: {}\n\
             If this directory is on a cloud-synced or network filesystem, \
             check that the sync service is running and responsive.",
            self.data_dir.display()
        )
    }

    #[must_use]
    pub fn declarations_dir(&self) -> &Path {
        &self.declarations_dir
    }

    #[must_use]
    pub fn flex_queries_dir(&self) -> &Path {
        &self.flex_queries_dir
    }

    // =======================================================================
    // Transactions
    // =======================================================================

    /// Read transactions from disk. Returns empty vec on missing / invalid file.
    #[must_use]
    pub fn load_transactions(&self) -> Vec<Transaction> {
        if !self.transactions_file.exists() {
            return Vec::new();
        }
        match std::fs::read_to_string(&self.transactions_file) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    pub fn write_transactions(&self, txns: &[Transaction]) -> Result<()> {
        let json = serde_json::to_string_pretty(txns)?;
        std::fs::write(&self.transactions_file, &json)?;
        Ok(())
    }

    /// Get transactions optionally filtered by date range.
    #[must_use]
    pub fn get_transactions(
        &self,
        from_date: Option<NaiveDate>,
        to_date: Option<NaiveDate>,
    ) -> Vec<Transaction> {
        let mut txns = self.load_transactions();
        if let Some(from) = from_date {
            txns.retain(|t| t.date >= from);
        }
        if let Some(to) = to_date {
            txns.retain(|t| t.date <= to);
        }
        txns
    }

    /// Merge `new` transactions into the existing file.
    /// Returns `(inserted, updated)`.
    pub fn save_transactions(&self, new: &[Transaction]) -> Result<(usize, usize)> {
        if new.is_empty() {
            return Ok((0, 0));
        }

        let mut existing = self.load_transactions();
        if existing.is_empty() {
            let count = new.len();
            self.write_transactions(new)?;
            return Ok((count, 0));
        }

        let (inserted, updated) = merge_transactions(&mut existing, new);
        if inserted > 0 || updated > 0 {
            self.write_transactions(&existing)?;
        }
        Ok((inserted, updated))
    }

    #[must_use]
    pub fn get_last_transaction_date(&self) -> Option<NaiveDate> {
        self.load_transactions().iter().map(|t| t.date).max()
    }

    // =======================================================================
    // Exchange Rates
    // =======================================================================

    #[must_use]
    pub fn load_rates(&self) -> IndexMap<String, String> {
        if !self.rates_file.exists() {
            return IndexMap::new();
        }
        match std::fs::read_to_string(&self.rates_file) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => IndexMap::new(),
        }
    }

    pub fn save_exchange_rate(&self, rate: &ExchangeRate) -> Result<()> {
        let mut rates = self.load_rates();
        let key = format!(
            "{}_{}",
            rate.date.format("%Y-%m-%d"),
            serde_json::to_value(&rate.currency)?
                .as_str()
                .unwrap_or("USD")
        );
        rates.insert(key, rate.rate.to_string());
        self.write_rates(&rates)
    }

    pub fn write_rates(&self, rates: &IndexMap<String, String>) -> Result<()> {
        let json = serde_json::to_string_pretty(rates)?;
        std::fs::write(&self.rates_file, &json)?;
        Ok(())
    }

    pub fn get_exchange_rate(&self, date: NaiveDate, currency: &Currency) -> Option<ExchangeRate> {
        let rates = self.load_rates();
        let cur_str = serde_json::to_value(currency)
            .ok()?
            .as_str()
            .map(String::from)?;
        let key = format!("{}_{cur_str}", date.format("%Y-%m-%d"));
        let val = rates.get(&key)?;
        let rate = val.parse::<Decimal>().ok()?;
        Some(ExchangeRate {
            date,
            currency: currency.clone(),
            rate,
        })
    }

    /// Find the nearest cached rate for a currency on or before `date`,
    /// scanning up to `max_lookback` days back through the cache.
    /// Loads the rates file once and probes the in-memory map.
    #[must_use]
    pub fn find_nearest_cached_rate(
        &self,
        date: NaiveDate,
        currency: &Currency,
        max_lookback: u32,
    ) -> Option<ExchangeRate> {
        let rates = self.load_rates();
        let cur_str = serde_json::to_value(currency)
            .ok()?
            .as_str()
            .map(String::from)?;

        let mut target = date;
        for _ in 0..max_lookback {
            let key = format!("{}_{cur_str}", target.format("%Y-%m-%d"));
            if let Some(val) = rates.get(&key)
                && let Ok(rate) = val.parse::<Decimal>()
            {
                return Some(ExchangeRate {
                    date: target,
                    currency: currency.clone(),
                    rate,
                });
            }
            target -= chrono::Duration::days(1);
        }
        None
    }

    // =======================================================================
    // Declarations
    // =======================================================================

    fn load_declarations_file(&self) -> DeclarationsFile {
        if !self.declarations_file.exists() {
            return DeclarationsFile::default();
        }
        match std::fs::read_to_string(&self.declarations_file) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => DeclarationsFile::default(),
        }
    }

    fn save_declarations_file(&self, data: &DeclarationsFile) -> Result<()> {
        let json = serde_json::to_string_pretty(data)?;
        std::fs::write(&self.declarations_file, &json)?;
        Ok(())
    }

    pub fn save_declaration(&self, declaration: &Declaration) -> Result<()> {
        let mut file = self.load_declarations_file();

        let mut declarations: Vec<Declaration> = file
            .declarations
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();

        if let Some(idx) = declarations
            .iter()
            .position(|d| d.declaration_id == declaration.declaration_id)
        {
            declarations[idx] = declaration.clone();
        } else {
            declarations.push(declaration.clone());
        }

        file.declarations = declarations
            .iter()
            .filter_map(|d| serde_json::to_value(d).ok())
            .collect();

        self.save_declarations_file(&file)
    }

    #[must_use]
    pub fn get_declarations(
        &self,
        status: Option<&DeclarationStatus>,
        declaration_type: Option<&DeclarationType>,
    ) -> Vec<Declaration> {
        let file = self.load_declarations_file();
        let mut decls: Vec<Declaration> = file
            .declarations
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();

        if let Some(s) = status {
            decls.retain(|d| &d.status == s);
        }
        if let Some(t) = declaration_type {
            decls.retain(|d| &d.r#type == t);
        }
        decls
    }

    #[must_use]
    pub fn get_declaration(&self, declaration_id: &str) -> Option<Declaration> {
        self.get_declarations(None, None)
            .into_iter()
            .find(|d| d.declaration_id == declaration_id)
    }

    #[must_use]
    pub fn declaration_exists(&self, declaration_id: &str) -> bool {
        self.get_declaration(declaration_id).is_some()
    }

    pub fn update_declaration_status(
        &self,
        declaration_id: &str,
        status: DeclarationStatus,
        timestamp: chrono::NaiveDateTime,
    ) -> Result<()> {
        let mut decl = self
            .get_declaration(declaration_id)
            .ok_or_else(|| anyhow::anyhow!("Declaration {declaration_id} not found"))?;

        decl.status = status;
        if status == DeclarationStatus::Submitted {
            decl.submitted_at = Some(timestamp);
        }
        self.save_declaration(&decl)
    }

    #[must_use]
    pub fn get_last_declaration_date(&self) -> Option<NaiveDate> {
        let file = self.load_declarations_file();
        file.last_declaration_date
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
    }

    pub fn set_last_declaration_date(&self, date: NaiveDate) -> Result<()> {
        let mut file = self.load_declarations_file();
        file.last_declaration_date = Some(date.format("%Y-%m-%d").to_string());
        self.save_declarations_file(&file)
    }

    /// Save a flex query XML report with delta compression.
    pub fn save_raw_report(&self, xml_content: &str, report_date: NaiveDate) -> Result<()> {
        crate::storage_flex::save_raw_report_with_delta(
            &self.flex_queries_dir,
            xml_content,
            report_date,
        )
    }

    /// Restore a flex query XML report for a given date.
    #[must_use]
    pub fn restore_report(&self, report_date: NaiveDate) -> Option<String> {
        crate::storage_flex::restore_report(&self.flex_queries_dir, report_date)
    }
}

// ===========================================================================
// Transaction merge logic  (port of Python Storage._identify_updates etc.)
// ===========================================================================

/// Merge `new` transactions into `existing` in-place.
/// Returns `(inserted, updated)`.
pub fn merge_transactions(existing: &mut Vec<Transaction>, new: &[Transaction]) -> (usize, usize) {
    let mut existing_keys: HashMap<TransactionKey, usize> = HashMap::new();
    let mut existing_id_map: HashMap<TransactionKey, Vec<String>> = HashMap::new();

    for txn in existing.iter() {
        let k = txn.make_key();
        *existing_keys.entry(k.clone()).or_insert(0) += 1;
        existing_id_map
            .entry(k)
            .or_default()
            .push(txn.transaction_id.clone());
    }

    let existing_ids: HashSet<String> = existing.iter().map(|t| t.transaction_id.clone()).collect();
    let existing_by_id: HashMap<String, &Transaction> = existing
        .iter()
        .map(|t| (t.transaction_id.clone(), t))
        .collect();

    let official_new_dates = get_official_dates(new);

    let mut ids_to_remove: HashSet<String> = HashSet::new();

    // XML supremacy: remove existing CSVs for dates covered by new XML records
    if !official_new_dates.is_empty() {
        for txn in existing.iter() {
            if txn.is_csv_sourced() {
                let d = txn.date.format("%Y-%m-%d").to_string();
                if official_new_dates.contains(&d) {
                    ids_to_remove.insert(txn.transaction_id.clone());
                }
            }
        }
    }

    let official_existing_dates = get_official_dates_from_slice(existing);

    let mut to_add: Vec<Transaction> = Vec::new();
    let mut updates_count: usize = 0;

    for txn in new {
        let new_id = &txn.transaction_id;

        if existing_ids.contains(new_id) {
            if let Some(existing_txn) = existing_by_id.get(new_id)
                && txn.is_identical_to(existing_txn)
            {
                continue;
            }
            to_add.push(txn.clone());
            updates_count += 1;
            continue;
        }

        let k = txn.make_key();
        let count = existing_keys.get(&k).copied().unwrap_or(0);
        if count > 0 {
            let is_new_csv = txn.is_csv_sourced();
            let matched_id = existing_id_map
                .get(&k)
                .and_then(|ids| ids.first())
                .cloned()
                .unwrap_or_default();
            let is_old_csv = matched_id.starts_with("csv-");

            if !is_new_csv && is_old_csv {
                ids_to_remove.insert(matched_id);
                to_add.push(txn.clone());
                consume_match(&mut existing_keys, &mut existing_id_map, &k);
                updates_count += 1;
            } else if is_new_csv && !is_old_csv {
                consume_match(&mut existing_keys, &mut existing_id_map, &k);
            } else if !is_new_csv && !is_old_csv {
                to_add.push(txn.clone());
                // XML vs XML split order -- don't consume
            } else {
                consume_match(&mut existing_keys, &mut existing_id_map, &k);
            }
        } else {
            // No match
            let is_new_csv = txn.is_csv_sourced();
            let d = txn.date.format("%Y-%m-%d").to_string();
            if !(is_new_csv && official_existing_dates.contains(&d)) {
                to_add.push(txn.clone());
            }
        }
    }

    if to_add.is_empty() && ids_to_remove.is_empty() {
        return (0, 0);
    }

    let inserted = to_add.len().saturating_sub(updates_count);

    // Remove IDs and existing records that share IDs with to_add
    let new_ids: HashSet<String> = to_add.iter().map(|t| t.transaction_id.clone()).collect();
    existing.retain(|t| {
        !ids_to_remove.contains(&t.transaction_id) && !new_ids.contains(&t.transaction_id)
    });
    existing.extend(to_add);

    (inserted, updates_count)
}

fn consume_match(
    keys: &mut HashMap<TransactionKey, usize>,
    id_map: &mut HashMap<TransactionKey, Vec<String>>,
    k: &TransactionKey,
) {
    if let Some(count) = keys.get_mut(k) {
        *count = count.saturating_sub(1);
    }
    if let Some(ids) = id_map.get_mut(k)
        && !ids.is_empty()
    {
        ids.remove(0);
    }
}

fn get_official_dates(txns: &[Transaction]) -> HashSet<String> {
    txns.iter()
        .filter(|t| !t.is_csv_sourced())
        .map(|t| t.date.format("%Y-%m-%d").to_string())
        .collect()
}

fn get_official_dates_from_slice(txns: &[Transaction]) -> HashSet<String> {
    get_official_dates(txns)
}
