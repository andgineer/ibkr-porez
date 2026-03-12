use chrono::{NaiveDate, NaiveDateTime};
use indexmap::IndexMap;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransactionType {
    #[serde(rename = "TRADE")]
    Trade,
    #[serde(rename = "DIVIDEND")]
    Dividend,
    #[serde(rename = "TAX")]
    Tax,
    #[serde(rename = "WITHHOLDING_TAX")]
    WithholdingTax,
    #[serde(rename = "INTEREST")]
    Interest,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AssetClass {
    #[serde(rename = "STK")]
    Stock,
    #[serde(rename = "OPT")]
    Opt,
    #[serde(rename = "CFD")]
    Cfd,
    #[serde(rename = "BOND")]
    Bond,
    #[serde(rename = "CASH")]
    Cash,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Currency {
    USD,
    EUR,
    GBP,
    RSD,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DeclarationType {
    #[serde(rename = "PPDG-3R")]
    Ppdg3r,
    #[serde(rename = "PP OPO")]
    Ppo,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeclarationStatus {
    #[serde(rename = "draft")]
    Draft,
    #[serde(rename = "submitted")]
    Submitted,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "finalized")]
    Finalized,
}

impl DeclarationStatus {
    #[must_use] 
    pub fn draft_default() -> Self {
        Self::Draft
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const INCOME_CODE_DIVIDEND: &str = "111402000";
pub const INCOME_CODE_COUPON: &str = "111403000";

pub const DECLARATION_STATUS_SCOPES: &[(&str, &[DeclarationStatus])] = &[
    (
        "Active",
        &[
            DeclarationStatus::Draft,
            DeclarationStatus::Submitted,
            DeclarationStatus::Pending,
        ],
    ),
    (
        "Pending payment",
        &[DeclarationStatus::Submitted, DeclarationStatus::Pending],
    ),
];

// ---------------------------------------------------------------------------
// Custom serde deserializers (flexible readers for Python-generated files)
// ---------------------------------------------------------------------------

use serde::Deserializer;
use std::str::FromStr;

fn parse_decimal_value(value: &serde_json::Value) -> Result<Decimal, String> {
    match value {
        serde_json::Value::String(s) => Decimal::from_str(s).map_err(|e| e.to_string()),
        serde_json::Value::Number(n) => Decimal::from_str(&n.to_string()).map_err(|e| e.to_string()),
        _ => Err("expected string or number".into()),
    }
}

fn parse_date_str(s: &str) -> Result<NaiveDate, String> {
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d);
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Ok(dt.date());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt.date());
    }
    Err(format!("invalid date: {s}"))
}

pub fn deserialize_decimal<'de, D: Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    parse_decimal_value(&v).map_err(serde::de::Error::custom)
}

pub fn deserialize_decimal_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    match v {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(ref val) => parse_decimal_value(val).map(Some).map_err(serde::de::Error::custom),
    }
}

pub fn deserialize_date<'de, D: Deserializer<'de>>(d: D) -> Result<NaiveDate, D::Error> {
    let s = String::deserialize(d)?;
    parse_date_str(&s).map_err(serde::de::Error::custom)
}

pub fn deserialize_date_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NaiveDate>, D::Error> {
    let v = Option::<String>::deserialize(d)?;
    match v {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => parse_date_str(&s).map(Some).map_err(serde::de::Error::custom),
    }
}

// ---------------------------------------------------------------------------
// Structs  (field order matches Python models.py exactly)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Transaction {
    pub transaction_id: String,
    #[serde(deserialize_with = "deserialize_date")]
    pub date: NaiveDate,
    pub r#type: TransactionType,
    pub symbol: String,
    pub description: String,
    #[serde(default, deserialize_with = "deserialize_decimal")]
    pub quantity: Decimal,
    #[serde(default, deserialize_with = "deserialize_decimal")]
    pub price: Decimal,
    #[serde(deserialize_with = "deserialize_decimal")]
    pub amount: Decimal,
    pub currency: Currency,
    #[serde(default, deserialize_with = "deserialize_date_opt")]
    pub open_date: Option<NaiveDate>,
    #[serde(default, deserialize_with = "deserialize_decimal_opt")]
    pub open_price: Option<Decimal>,
    #[serde(default, deserialize_with = "deserialize_decimal_opt")]
    pub exchange_rate: Option<Decimal>,
    #[serde(default, deserialize_with = "deserialize_decimal_opt")]
    pub amount_rsd: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExchangeRate {
    pub date: NaiveDate,
    pub currency: Currency,
    pub rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TaxReportEntry {
    pub ticker: String,
    pub quantity: Decimal,
    pub sale_date: NaiveDate,
    pub sale_price: Decimal,
    pub sale_exchange_rate: Decimal,
    pub sale_value_rsd: Decimal,
    pub purchase_date: NaiveDate,
    pub purchase_price: Decimal,
    pub purchase_exchange_rate: Decimal,
    pub purchase_value_rsd: Decimal,
    pub capital_gain_rsd: Decimal,
    #[serde(default)]
    pub is_tax_exempt: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IncomeEntry {
    pub date: NaiveDate,
    pub symbol: String,
    pub amount: Decimal,
    pub currency: Currency,
    pub amount_rsd: Decimal,
    pub exchange_rate: Decimal,
    pub income_type: String,
    pub description: String,
    #[serde(default)]
    pub withholding_tax_usd: Decimal,
    #[serde(default)]
    pub withholding_tax_rsd: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IncomeDeclarationEntry {
    pub date: NaiveDate,
    pub symbol_or_currency: Option<String>,
    pub sifra_vrste_prihoda: String,
    pub bruto_prihod: Decimal,
    pub osnovica_za_porez: Decimal,
    pub obracunati_porez: Decimal,
    pub porez_placen_drugoj_drzavi: Decimal,
    pub porez_za_uplatu: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Declaration {
    pub declaration_id: String,
    pub r#type: DeclarationType,
    #[serde(default = "DeclarationStatus::draft_default")]
    pub status: DeclarationStatus,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub created_at: NaiveDateTime,
    pub submitted_at: Option<NaiveDateTime>,
    pub paid_at: Option<NaiveDateTime>,
    pub file_path: Option<String>,
    pub xml_content: Option<String>,
    pub report_data: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub metadata: IndexMap<String, serde_json::Value>,
    #[serde(default)]
    pub attached_files: IndexMap<String, String>,
}

/// Top-level structure of `declarations.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[derive(Default)]
pub struct DeclarationsFile {
    pub declarations: Vec<serde_json::Value>,
    pub last_declaration_date: Option<String>,
}


fn default_city_code() -> String {
    "223".into()
}
fn default_phone() -> String {
    "0600000000".into()
}
fn default_email() -> String {
    "email@example.com".into()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserConfig {
    #[serde(default)]
    pub ibkr_token: String,
    #[serde(default)]
    pub ibkr_query_id: String,
    #[serde(default)]
    pub personal_id: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default = "default_city_code")]
    pub city_code: String,
    #[serde(default = "default_phone")]
    pub phone: String,
    #[serde(default = "default_email")]
    pub email: String,
    pub data_dir: Option<String>,
    pub output_folder: Option<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            ibkr_token: String::new(),
            ibkr_query_id: String::new(),
            personal_id: String::new(),
            full_name: String::new(),
            address: String::new(),
            city_code: default_city_code(),
            phone: default_phone(),
            email: default_email(),
            data_dir: None,
            output_folder: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Transaction helpers for merge logic
// ---------------------------------------------------------------------------

/// Dedup key: (`date_str`, symbol, `type_value`, `abs_quantity_f64`, `rounded_price_f64`).
pub type TransactionKey = (String, String, String, u64, i64);

impl Transaction {
    #[must_use]
    pub fn make_key(&self) -> TransactionKey {
        let date_str = self.date.format("%Y-%m-%d").to_string();
        let type_value = serde_json::to_value(&self.r#type)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let q = self.quantity.abs().to_string().parse::<f64>().unwrap_or(0.0);
        let p = self.price.to_string().parse::<f64>().unwrap_or(0.0);
        #[allow(clippy::cast_possible_truncation)]
        let p_rounded = (p * 10000.0).round() as i64;
        (date_str, self.symbol.clone(), type_value, q.to_bits(), p_rounded)
    }

    #[must_use]
    pub fn is_csv_sourced(&self) -> bool {
        self.transaction_id.starts_with("csv-")
    }

    #[must_use]
    pub fn is_identical_to(&self, other: &Self) -> bool {
        let float_tolerance = 0.0001;

        if self.date != other.date {
            return false;
        }

        let self_fields = [&self.quantity, &self.price, &self.amount];
        let other_fields = [&other.quantity, &other.price, &other.amount];
        for (a, b) in self_fields.iter().zip(other_fields.iter()) {
            let va: f64 = a.to_string().parse().unwrap_or(0.0);
            let vb: f64 = b.to_string().parse().unwrap_or(0.0);
            if (va - vb).abs() > float_tolerance {
                return false;
            }
        }

        let str_eq = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
        if !str_eq(&self.symbol, &other.symbol) {
            return false;
        }
        if !str_eq(&self.description, &other.description) {
            return false;
        }

        let self_type = serde_json::to_value(&self.r#type).unwrap_or_default();
        let other_type = serde_json::to_value(&other.r#type).unwrap_or_default();
        if self_type != other_type {
            return false;
        }

        let self_cur = serde_json::to_value(&self.currency).unwrap_or_default();
        let other_cur = serde_json::to_value(&other.currency).unwrap_or_default();
        self_cur == other_cur
    }
}
