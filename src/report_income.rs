use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use anyhow::Result;
use chrono::{Duration, NaiveDate};
use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use tracing::debug;

use crate::declaration_income_xml::generate_income_xml;
use crate::holidays::HolidayCalendar;
use crate::models::{
    Currency, INCOME_CODE_COUPON, INCOME_CODE_DIVIDEND, IncomeDeclarationEntry, Transaction,
    TransactionType, UserConfig,
};
use crate::nbs::NBSClient;

/// A group that matched no withholding row at all is held this long before it
/// is declared with a zero credit. The PP-OPO deadline is 30 days from the
/// income date, so this leaves 23 days to file.
pub const WHT_WAIT_DAYS: i64 = 7;

/// `(date, SYMBOL-or-CURRENCY upper, income_type)`
pub type GroupKey = (NaiveDate, String, String);

const PER_SHARE: &str = "PER SHARE";

static INTEREST_MONTH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"CREDIT INT FOR [A-Z]{3}-\d{4}").unwrap());

/// One income group in the broker's own currency. No exchange rate involved.
#[derive(Debug, Clone)]
pub struct IncomeGroup {
    pub key: GroupKey,
    /// The first income row's; a group is one currency.
    pub currency: Currency,
    /// Signed sum of the group's income rows.
    pub gross_ccy: Decimal,
    /// Credit, clamped to zero from below.
    pub tax_ccy: Decimal,
    /// A withholding row joined, whatever it summed to.
    pub matched_any_tax: bool,
}

/// The metadata keys holding a declaration's source amounts, named once so the
/// writer here and the reader in `sync.rs` cannot drift apart on a spelling.
const KEY_GROSS_CCY: &str = "gross_income_ccy";
const KEY_TAX_CCY: &str = "foreign_tax_paid_ccy";
const KEY_CURRENCY: &str = "currency";
const KEY_EXCHANGE_RATE: &str = "exchange_rate";
const KEY_AMENDS: &str = "amends";

/// What a declaration records about its own source, in the income currency.
/// Formatted as stored: the comparison is on these strings, which fixes the
/// threshold at 0.01 and keeps a moved exchange rate from looking like a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAmounts {
    pub gross_ccy: String,
    pub tax_ccy: String,
    pub currency: String,
}

impl SourceAmounts {
    #[must_use]
    pub fn of(group: &IncomeGroup) -> Self {
        Self {
            gross_ccy: format!("{:.2}", group.gross_ccy),
            tax_ccy: format!("{:.2}", group.tax_ccy),
            currency: group.currency.as_code().to_string(),
        }
    }

    pub fn to_metadata(&self, m: &mut indexmap::IndexMap<String, serde_json::Value>) {
        m.insert(KEY_GROSS_CCY.into(), self.gross_ccy.clone().into());
        m.insert(KEY_TAX_CCY.into(), self.tax_ccy.clone().into());
        m.insert(KEY_CURRENCY.into(), self.currency.clone().into());
    }

    #[must_use]
    pub fn from_metadata(m: &indexmap::IndexMap<String, serde_json::Value>) -> Option<Self> {
        let get = |key: &str| m.get(key).and_then(|v| v.as_str()).map(str::to_string);
        Some(Self {
            gross_ccy: get(KEY_GROSS_CCY)?,
            tax_ccy: get(KEY_TAX_CCY)?,
            currency: get(KEY_CURRENCY)?,
        })
    }
}

/// Whether this group already has a declaration, and whether that declaration
/// can be compared at all. A declaration carrying no `SourceAmounts` is not one
/// these rules are defined over, and `YesWithoutRecord` says so in the type.
#[derive(Debug, Clone, Copy)]
pub enum Declared<'a> {
    No,
    Yes(&'a SourceAmounts),
    YesWithoutRecord,
}

/// The declaration an amendment replaces.
pub struct Amends {
    pub declaration_id: String,
    /// The PURS number recorded at `submit`, written as `IdentifikatorPrijave`.
    pub purs_number: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupAction {
    Create,
    Amend,
    Skip,
    Wait,
}

pub struct RenderOptions {
    pub today: NaiveDate,
    pub force_rates: bool,
}

pub struct IncomeReport {
    pub filename: String,
    pub xml_content: String,
    pub entries: Vec<IncomeDeclarationEntry>,
    pub declaration_date: NaiveDate,
    /// The group's own figures. `entries` hold RSD only, so without these
    /// there would be nothing for `metadata()` to record.
    pub source: SourceAmounts,
    pub exchange_rate: Decimal,
    /// The `declaration_id` this report amends.
    pub amends: Option<String>,
}

impl IncomeReport {
    #[must_use]
    pub fn metadata(&self) -> indexmap::IndexMap<String, serde_json::Value> {
        let mut m = indexmap::IndexMap::new();
        m.insert("entry_count".into(), self.entries.len().into());

        let income_type = if self
            .entries
            .first()
            .is_some_and(|e| e.sifra_vrste_prihoda == INCOME_CODE_DIVIDEND)
        {
            "dividend"
        } else {
            "coupon"
        };
        m.insert("income_type".into(), income_type.into());

        let symbol = self
            .entries
            .first()
            .and_then(|e| e.symbol_or_currency.as_deref())
            .unwrap_or("")
            .to_uppercase();
        m.insert("symbol".into(), symbol.into());

        let dates: Vec<NaiveDate> = self.entries.iter().map(|e| e.date).collect();
        if let (Some(min), Some(max)) = (dates.iter().min(), dates.iter().max()) {
            m.insert("period_start".into(), fmt_date(*min).into());
            m.insert("period_end".into(), fmt_date(*max).into());
        }

        let gross: Decimal = self.entries.iter().map(|e| e.bruto_prihod).sum();
        let tax_base: Decimal = self.entries.iter().map(|e| e.osnovica_za_porez).sum();
        let calc_tax: Decimal = self.entries.iter().map(|e| e.obracunati_porez).sum();
        let foreign: Decimal = self
            .entries
            .iter()
            .map(|e| e.porez_placen_drugoj_drzavi)
            .sum();
        let due: Decimal = self.entries.iter().map(|e| e.porez_za_uplatu).sum();

        m.insert("gross_income_rsd".into(), format!("{gross:.2}").into());
        m.insert("tax_base_rsd".into(), format!("{tax_base:.2}").into());
        m.insert("calculated_tax_rsd".into(), format!("{calc_tax:.2}").into());
        m.insert(
            "foreign_tax_paid_rsd".into(),
            format!("{foreign:.2}").into(),
        );
        m.insert("tax_due_rsd".into(), format!("{due:.2}").into());

        self.source.to_metadata(&mut m);
        m.insert(
            KEY_EXCHANGE_RATE.into(),
            self.exchange_rate.to_string().into(),
        );
        if let Some(amends) = &self.amends {
            m.insert(KEY_AMENDS.into(), amends.clone().into());
        }
        m
    }
}

/// The distribution a transaction belongs to: a dividend and every withholding
/// row taken from it -- reversals included -- share this key.
fn distribution_key(txn: &Transaction) -> Option<String> {
    if let Some(action_id) = &txn.action_id {
        return Some(format!("action:{action_id}"));
    }

    let desc = txn.description.to_uppercase();
    if let Some(pos) = desc.find(PER_SHARE) {
        return Some(format!("desc:{}", &desc[..pos + PER_SHARE.len()]));
    }

    INTEREST_MONTH_RE
        .find(&desc)
        .map(|m| format!("int:{}:{}", txn.currency.as_code(), m.as_str()))
}

struct GroupBuilder {
    currency: Currency,
    gross_ccy: Decimal,
    distribution_keys: Vec<String>,
}

/// Group income in `[scan_start, end]` and join each group's withholding tax by
/// distribution. Pure: no storage, no exchange rate, no clock.
#[must_use]
pub fn collect_income_groups(
    txns: &[Transaction],
    scan_start: NaiveDate,
    end: NaiveDate,
) -> Vec<IncomeGroup> {
    let mut taxes: HashMap<String, Vec<&Transaction>> = HashMap::new();
    for txn in txns.iter().filter(|t| {
        t.r#type == TransactionType::WithholdingTax && t.date >= scan_start && !t.is_csv_sourced()
    }) {
        if let Some(key) = distribution_key(txn) {
            taxes.entry(key).or_default().push(txn);
        }
    }

    let mut builders: BTreeMap<GroupKey, GroupBuilder> = BTreeMap::new();
    for txn in txns
        .iter()
        .filter(|t| t.date >= scan_start && t.date <= end && !t.is_csv_sourced())
    {
        let income_type = match txn.r#type {
            TransactionType::Dividend => "dividend",
            TransactionType::Interest => "coupon",
            _ => continue,
        };
        let sym_or_currency = if income_type == "dividend" {
            txn.symbol.to_uppercase()
        } else {
            txn.currency.as_code().to_uppercase()
        };

        let builder = builders
            .entry((txn.date, sym_or_currency, income_type.to_string()))
            .or_insert_with(|| GroupBuilder {
                currency: txn.currency.clone(),
                gross_ccy: Decimal::ZERO,
                distribution_keys: Vec::new(),
            });
        builder.gross_ccy += txn.amount;
        if let Some(key) = distribution_key(txn)
            && !builder.distribution_keys.contains(&key)
        {
            builder.distribution_keys.push(key);
        }
    }

    let mut owners: HashMap<String, usize> = HashMap::new();
    for builder in builders.values() {
        for key in &builder.distribution_keys {
            *owners.entry(key.clone()).or_default() += 1;
        }
    }

    builders
        .into_iter()
        .map(|(key, builder)| {
            let mut signed_tax = Decimal::ZERO;
            let mut matched_any_tax = false;
            for distribution in &builder.distribution_keys {
                if owners[distribution] > 1 {
                    continue;
                }
                if let Some(rows) = taxes.get(distribution) {
                    matched_any_tax = true;
                    signed_tax += rows.iter().map(|t| t.amount).sum::<Decimal>();
                }
            }
            // Not `max(ZERO)`: negating a zero sum yields a negative zero, which
            // compares equal to zero but formats as `-0.00`.
            let credit = -signed_tax;
            IncomeGroup {
                key,
                currency: builder.currency,
                gross_ccy: builder.gross_ccy,
                tax_ccy: if credit > Decimal::ZERO {
                    credit
                } else {
                    Decimal::ZERO
                },
                matched_any_tax,
            }
        })
        .collect()
}

/// What to do with one group. Pure: every timing rule is decided here.
#[must_use]
pub fn decide(
    group: &IncomeGroup,
    declared: Declared<'_>,
    creation_start: NaiveDate,
    today: NaiveDate,
) -> GroupAction {
    if group.gross_ccy <= Decimal::ZERO {
        return GroupAction::Skip;
    }

    match declared {
        Declared::Yes(recorded) => {
            if *recorded == SourceAmounts::of(group) {
                GroupAction::Skip
            } else {
                GroupAction::Amend
            }
        }
        Declared::YesWithoutRecord => GroupAction::Skip,
        Declared::No => {
            let date = group.key.0;
            if date < creation_start {
                return GroupAction::Skip;
            }
            if !group.matched_any_tax && date + Duration::days(WHT_WAIT_DAYS) >= today {
                return GroupAction::Wait;
            }
            GroupAction::Create
        }
    }
}

/// The day a waiting group stops waiting and is declared with a zero credit.
#[must_use]
pub fn wait_expires_on(group: &IncomeGroup) -> NaiveDate {
    group.key.0 + Duration::days(WHT_WAIT_DAYS + 1)
}

#[must_use]
pub fn income_report_filename(key: &GroupKey, is_amendment: bool) -> String {
    let (date, sym_or_currency, _) = key;
    let izmena = if is_amendment { "izmena-" } else { "" };
    format!(
        "ppopo-{izmena}{}-{}.xml",
        sym_or_currency.to_lowercase(),
        date.format("%Y-%m%d")
    )
}

/// Resolve the exchange rate and build the PP-OPO document for one group.
pub fn render_income_report(
    group: &IncomeGroup,
    amends: Option<&Amends>,
    nbs: &NBSClient,
    config: &UserConfig,
    holidays: &HolidayCalendar,
    opts: &RenderOptions,
) -> Result<IncomeReport> {
    let (date, sym_or_currency, income_type) = &group.key;

    let rate = get_rate_or_force(nbs, *date, &group.currency, opts.force_rates)?;

    let total_bruto = round2(group.gross_ccy * rate);
    let porez_placen = round2(group.tax_ccy * rate);
    let obracunati = round2_half_up(total_bruto * Decimal::new(15, 2));

    let decl_entry = IncomeDeclarationEntry {
        date: *date,
        symbol_or_currency: Some(sym_or_currency.clone()),
        sifra_vrste_prihoda: if income_type == "dividend" {
            INCOME_CODE_DIVIDEND.to_string()
        } else {
            INCOME_CODE_COUPON.to_string()
        },
        bruto_prihod: total_bruto,
        osnovica_za_porez: total_bruto,
        obracunati_porez: obracunati,
        porez_placen_drugoj_drzavi: porez_placen,
        porez_za_uplatu: round2((obracunati - porez_placen).max(Decimal::ZERO)),
    };

    let xml = generate_income_xml(&decl_entry, config, holidays, opts.today, amends);
    let filename = income_report_filename(&group.key, amends.is_some());

    debug!(%filename, %total_bruto, %porez_placen, "generated income declaration");

    Ok(IncomeReport {
        filename,
        xml_content: xml,
        entries: vec![decl_entry],
        declaration_date: *date,
        source: SourceAmounts::of(group),
        exchange_rate: rate,
        amends: amends.map(|a| a.declaration_id.clone()),
    })
}

const FORCE_LOOKBACK_DAYS: u32 = 365;

fn get_rate_or_force(
    nbs: &NBSClient,
    date: NaiveDate,
    currency: &Currency,
    force: bool,
) -> Result<Decimal> {
    if *currency == Currency::RSD {
        return Ok(Decimal::ONE);
    }
    match nbs.get_rate(date, currency) {
        Ok(Some(rate)) => Ok(rate),
        Ok(None) if force => force_fallback_rate(nbs, date, currency),
        Ok(None) => Err(anyhow::anyhow!(
            "no NBS exchange rate for {currency:?} on {date}"
        )),
        Err(e) if force => {
            tracing::warn!(%date, ?currency, error = %e, "rate error in force mode, trying cache");
            force_fallback_rate(nbs, date, currency)
        }
        Err(e) => Err(e),
    }
}

fn force_fallback_rate(nbs: &NBSClient, date: NaiveDate, currency: &Currency) -> Result<Decimal> {
    if let Some(cached) =
        nbs.storage()
            .find_nearest_cached_rate(date, currency, FORCE_LOOKBACK_DAYS)
    {
        tracing::warn!(
            %date, ?currency, fallback_date = %cached.date, rate = %cached.rate,
            "force mode: using nearest cached rate"
        );
        Ok(cached.rate)
    } else {
        tracing::warn!(%date, ?currency, "force mode: no cached rate found at all");
        Err(anyhow::anyhow!(
            "no NBS exchange rate found for {currency:?} on {date} (even with force lookback)"
        ))
    }
}

fn round2(d: Decimal) -> Decimal {
    d.round_dp(2)
}

fn round2_half_up(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn txn(
        id: &str,
        r#type: TransactionType,
        symbol: &str,
        on: &str,
        amount: Decimal,
        description: &str,
    ) -> Transaction {
        Transaction {
            transaction_id: id.to_string(),
            date: date(on),
            r#type,
            symbol: symbol.to_string(),
            description: description.to_string(),
            quantity: Decimal::ZERO,
            price: Decimal::ZERO,
            amount,
            currency: Currency::USD,
            open_date: None,
            open_price: None,
            exchange_rate: None,
            amount_rsd: None,
            action_id: None,
        }
    }

    fn with_action_id(mut t: Transaction, action_id: &str) -> Transaction {
        t.action_id = Some(action_id.to_string());
        t
    }

    fn with_currency(mut t: Transaction, currency: Currency) -> Transaction {
        t.currency = currency;
        t
    }

    fn dividend_desc(symbol: &str, isin: &str, per_share: &str) -> String {
        format!("{symbol}({isin}) CASH DIVIDEND USD {per_share} PER SHARE (Ordinary Dividend)")
    }

    fn tax_desc(symbol: &str, isin: &str, per_share: &str) -> String {
        format!("{symbol}({isin}) CASH DIVIDEND USD {per_share} PER SHARE - US TAX")
    }

    fn collect(txns: &[Transaction]) -> Vec<IncomeGroup> {
        collect_income_groups(txns, date("2000-01-01"), date("2099-12-31"))
    }

    fn only(txns: &[Transaction]) -> IncomeGroup {
        let groups = collect(txns);
        assert_eq!(groups.len(), 1, "expected exactly one group: {groups:?}");
        groups.into_iter().next().unwrap()
    }

    // -----------------------------------------------------------------
    // Matching and netting -- collect_income_groups
    // -----------------------------------------------------------------

    #[test]
    fn wht_matched_by_action_id() {
        let txns = vec![
            with_action_id(
                txn(
                    "d1",
                    TransactionType::Dividend,
                    "SGOV",
                    "2025-12-19",
                    dec!(40.82),
                    &dividend_desc("SGOV", "US46436E7186", "1.570153"),
                ),
                "111",
            ),
            with_action_id(
                txn(
                    "w1",
                    TransactionType::WithholdingTax,
                    "SGOV",
                    "2025-12-19",
                    dec!(-12.25),
                    &tax_desc("SGOV", "US46436E7186", "1.570153"),
                ),
                "111",
            ),
            with_action_id(
                txn(
                    "d2",
                    TransactionType::Dividend,
                    "SGOV",
                    "2025-12-24",
                    dec!(87.55),
                    &dividend_desc("SGOV", "US46436E7186", "1.570153"),
                ),
                "222",
            ),
            with_action_id(
                txn(
                    "w2",
                    TransactionType::WithholdingTax,
                    "SGOV",
                    "2025-12-24",
                    dec!(-26.27),
                    &tax_desc("SGOV", "US46436E7186", "1.570153"),
                ),
                "222",
            ),
        ];

        let groups = collect(&txns);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].tax_ccy, dec!(12.25));
        assert_eq!(groups[1].tax_ccy, dec!(26.27));
    }

    #[test]
    fn wht_matched_by_description_when_action_id_absent() {
        let group = only(&[
            txn(
                "d1",
                TransactionType::Dividend,
                "VOO",
                "2025-10-01",
                dec!(17.40),
                &dividend_desc("VOO", "US9229083632", "1.74"),
            ),
            txn(
                "w1",
                TransactionType::WithholdingTax,
                "VOO",
                "2025-10-01",
                dec!(-5.22),
                &tax_desc("VOO", "US9229083632", "1.74"),
            ),
        ]);

        assert_eq!(group.tax_ccy, dec!(5.22));
        assert!(group.matched_any_tax);
    }

    #[test]
    fn wht_matched_by_interest_token() {
        let group = only(&[
            txn(
                "i1",
                TransactionType::Interest,
                "",
                "2025-08-05",
                dec!(0.03),
                "USD CREDIT INT FOR JUL-2025",
            ),
            txn(
                "w1",
                TransactionType::WithholdingTax,
                "",
                "2025-08-05",
                dec!(-0.01),
                "WITHHOLDING @ 30% ON CREDIT INT FOR JUL-2025",
            ),
        ]);

        assert_eq!(group.key.1, "USD");
        assert_eq!(group.tax_ccy, dec!(0.01));
    }

    #[test]
    fn interest_tax_does_not_cross_currencies() {
        let txns = vec![
            txn(
                "i1",
                TransactionType::Interest,
                "",
                "2025-08-05",
                dec!(0.03),
                "USD CREDIT INT FOR JUL-2025",
            ),
            with_currency(
                txn(
                    "i2",
                    TransactionType::Interest,
                    "",
                    "2025-08-05",
                    dec!(0.05),
                    "EUR CREDIT INT FOR JUL-2025",
                ),
                Currency::EUR,
            ),
            txn(
                "w1",
                TransactionType::WithholdingTax,
                "",
                "2025-08-05",
                dec!(-0.01),
                "WITHHOLDING @ 30% ON CREDIT INT FOR JUL-2025",
            ),
        ];

        let groups = collect(&txns);
        assert_eq!(groups.len(), 2);

        let usd = groups.iter().find(|g| g.key.1 == "USD").unwrap();
        let eur = groups.iter().find(|g| g.key.1 == "EUR").unwrap();
        assert_eq!(usd.tax_ccy, dec!(0.01));
        assert_eq!(eur.tax_ccy, Decimal::ZERO);
        assert!(!eur.matched_any_tax);
    }

    #[test]
    fn wht_across_year_end_is_credited() {
        let group = only(&[
            txn(
                "d1",
                TransactionType::Dividend,
                "VOO",
                "2025-12-24",
                dec!(100),
                &dividend_desc("VOO", "US9229083632", "1.771"),
            ),
            txn(
                "w1",
                TransactionType::WithholdingTax,
                "VOO",
                "2026-01-02",
                dec!(-15),
                &tax_desc("VOO", "US9229083632", "1.771"),
            ),
        ]);

        assert_eq!(group.tax_ccy, dec!(15));
    }

    #[test]
    fn reversed_income_nets_to_zero() {
        let group = only(&[
            txn(
                "d1",
                TransactionType::Dividend,
                "VOO",
                "2025-12-24",
                dec!(21.25),
                &dividend_desc("VOO", "US9229083632", "1.771"),
            ),
            txn(
                "d2",
                TransactionType::Dividend,
                "VOO",
                "2025-12-24",
                dec!(-21.25),
                &dividend_desc("VOO", "US9229083632", "1.771"),
            ),
        ]);

        assert_eq!(group.gross_ccy, Decimal::ZERO);
    }

    #[test]
    fn wht_reversal_nets_to_zero() {
        let group = only(&[
            txn(
                "d1",
                TransactionType::Dividend,
                "SGOV",
                "2025-12-24",
                dec!(87.55),
                &dividend_desc("SGOV", "US46436E7186", "0.323046"),
            ),
            txn(
                "w1",
                TransactionType::WithholdingTax,
                "SGOV",
                "2025-12-24",
                dec!(-26.27),
                &tax_desc("SGOV", "US46436E7186", "0.323046"),
            ),
            txn(
                "w2",
                TransactionType::WithholdingTax,
                "SGOV",
                "2025-12-24",
                dec!(26.27),
                &tax_desc("SGOV", "US46436E7186", "0.323046"),
            ),
        ]);

        assert_eq!(group.tax_ccy, Decimal::ZERO);
        assert!(group.matched_any_tax);
        // A negated zero compares equal to zero but would be recorded as `-0.00`.
        assert_eq!(SourceAmounts::of(&group).tax_ccy, "0.00");
    }

    #[test]
    fn reversals_exceeding_withholding_clamp_to_zero() {
        let group = only(&[
            txn(
                "d1",
                TransactionType::Dividend,
                "SGOV",
                "2025-12-24",
                dec!(87.55),
                &dividend_desc("SGOV", "US46436E7186", "0.323046"),
            ),
            txn(
                "w1",
                TransactionType::WithholdingTax,
                "SGOV",
                "2025-12-24",
                dec!(-26.27),
                &tax_desc("SGOV", "US46436E7186", "0.323046"),
            ),
            txn(
                "w2",
                TransactionType::WithholdingTax,
                "SGOV",
                "2025-12-24",
                dec!(52.54),
                &tax_desc("SGOV", "US46436E7186", "0.323046"),
            ),
        ]);

        assert_eq!(group.tax_ccy, Decimal::ZERO);
    }

    #[test]
    fn csv_rows_are_excluded_on_both_sides() {
        let groups = collect(&[
            txn(
                "csv-d1",
                TransactionType::Dividend,
                "VOO",
                "2023-05-10",
                dec!(17.40),
                &dividend_desc("VOO", "US9229083632", "1.74"),
            ),
            txn(
                "csv-w1",
                TransactionType::WithholdingTax,
                "VOO",
                "2023-05-10",
                dec!(-5.22),
                &tax_desc("VOO", "US9229083632", "1.74"),
            ),
        ]);

        assert!(groups.is_empty());
    }

    #[test]
    fn ambiguous_key_credits_neither_group() {
        let groups = collect(&[
            txn(
                "d1",
                TransactionType::Dividend,
                "VOO",
                "2025-10-01",
                dec!(17.40),
                &dividend_desc("VOO", "US9229083632", "1.74"),
            ),
            txn(
                "d2",
                TransactionType::Dividend,
                "VOO",
                "2025-12-24",
                dec!(17.40),
                &dividend_desc("VOO", "US9229083632", "1.74"),
            ),
            txn(
                "w1",
                TransactionType::WithholdingTax,
                "VOO",
                "2025-10-01",
                dec!(-5.22),
                &tax_desc("VOO", "US9229083632", "1.74"),
            ),
        ]);

        assert_eq!(groups.len(), 2);
        for group in &groups {
            assert_eq!(group.tax_ccy, Decimal::ZERO);
            assert!(!group.matched_any_tax);
        }
    }

    #[test]
    fn group_key_is_upper_cased() {
        let group = only(&[txn(
            "d1",
            TransactionType::Dividend,
            "voo",
            "2025-10-01",
            dec!(17.40),
            &dividend_desc("VOO", "US9229083632", "1.74"),
        )]);

        assert_eq!(group.key.1, "VOO");
    }

    #[test]
    fn income_outside_the_scan_span_is_not_grouped() {
        let txns = vec![txn(
            "d1",
            TransactionType::Dividend,
            "VOO",
            "2025-10-01",
            dec!(17.40),
            &dividend_desc("VOO", "US9229083632", "1.74"),
        )];

        assert!(collect_income_groups(&txns, date("2025-10-02"), date("2025-12-31")).is_empty());
        assert!(collect_income_groups(&txns, date("2025-01-01"), date("2025-09-30")).is_empty());
    }

    // -----------------------------------------------------------------
    // Decisions -- decide
    // -----------------------------------------------------------------

    fn group_on(on: &str, gross: Decimal, tax: Decimal, matched_any_tax: bool) -> IncomeGroup {
        IncomeGroup {
            key: (date(on), "VOO".into(), "dividend".into()),
            currency: Currency::USD,
            gross_ccy: gross,
            tax_ccy: tax,
            matched_any_tax,
        }
    }

    fn recorded(gross_ccy: &str, tax_ccy: &str) -> SourceAmounts {
        SourceAmounts {
            gross_ccy: gross_ccy.into(),
            tax_ccy: tax_ccy.into(),
            currency: "USD".into(),
        }
    }

    #[test]
    fn no_wht_waits_while_window_open() {
        let group = group_on("2026-03-10", dec!(100), Decimal::ZERO, false);
        assert_eq!(
            decide(&group, Declared::No, date("2026-01-01"), date("2026-03-17")),
            GroupAction::Wait
        );
    }

    #[test]
    fn no_wht_finalizes_after_wait_elapses() {
        let group = group_on("2026-03-10", dec!(100), Decimal::ZERO, false);
        assert_eq!(
            decide(&group, Declared::No, date("2026-01-01"), date("2026-03-18")),
            GroupAction::Create
        );
        assert_eq!(wait_expires_on(&group), date("2026-03-18"));
    }

    #[test]
    fn netted_wht_does_not_wait() {
        let group = group_on("2026-03-10", dec!(100), Decimal::ZERO, true);
        assert_eq!(
            decide(&group, Declared::No, date("2026-01-01"), date("2026-03-11")),
            GroupAction::Create
        );
    }

    #[test]
    fn fully_reversed_income_is_never_declared() {
        let group = group_on("2026-03-10", Decimal::ZERO, Decimal::ZERO, true);
        assert_eq!(
            decide(&group, Declared::No, date("2026-01-01"), date("2026-03-20")),
            GroupAction::Skip
        );
    }

    #[test]
    fn already_declared_unchanged_is_skipped() {
        let group = group_on("2026-03-10", dec!(100), dec!(15), true);
        let declared = SourceAmounts::of(&group);
        assert_eq!(
            decide(
                &group,
                Declared::Yes(&declared),
                date("2026-01-01"),
                date("2026-03-20")
            ),
            GroupAction::Skip
        );
    }

    #[test]
    fn already_declared_changed_amends() {
        // The credit was reversed after the declaration was written.
        let group = group_on("2026-03-10", dec!(100), Decimal::ZERO, true);
        let declared = recorded("100.00", "15.00");
        assert_eq!(
            decide(
                &group,
                Declared::Yes(&declared),
                date("2026-01-01"),
                date("2026-03-20")
            ),
            GroupAction::Amend
        );
    }

    #[test]
    fn declaration_without_recorded_amounts_is_never_amended() {
        let group = group_on("2026-03-10", dec!(100), Decimal::ZERO, true);
        assert_eq!(
            decide(
                &group,
                Declared::YesWithoutRecord,
                date("2026-01-01"),
                date("2026-03-20")
            ),
            GroupAction::Skip
        );
    }

    #[test]
    fn outside_creation_window_is_skipped_when_undeclared() {
        let group = group_on("2025-07-02", dec!(100), dec!(15), true);
        assert_eq!(
            decide(&group, Declared::No, date("2026-01-01"), date("2026-03-20")),
            GroupAction::Skip
        );

        // Declared and since changed, the same group is still amended: the
        // creation window bounds what is created, not what is corrected.
        let declared = recorded("100.00", "0.00");
        assert_eq!(
            decide(
                &group,
                Declared::Yes(&declared),
                date("2026-01-01"),
                date("2026-03-20")
            ),
            GroupAction::Amend
        );
    }
}
