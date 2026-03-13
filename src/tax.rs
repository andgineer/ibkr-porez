use std::collections::{BTreeMap, VecDeque};

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use tracing::{debug, warn};

use crate::models::{Currency, TaxReportEntry, Transaction, TransactionType};
use crate::nbs::NBSClient;

const FORCE_LOOKBACK_DAYS: u32 = 365;

struct Lot {
    date: NaiveDate,
    price: Decimal,
    quantity: Decimal,
    #[allow(dead_code)]
    currency: Currency,
}

pub struct TaxCalculator<'a> {
    nbs: &'a NBSClient<'a>,
    force: bool,
}

impl<'a> TaxCalculator<'a> {
    #[must_use]
    pub fn new(nbs: &'a NBSClient<'a>) -> Self {
        Self { nbs, force: false }
    }

    #[must_use]
    pub fn with_force(nbs: &'a NBSClient<'a>, force: bool) -> Self {
        Self { nbs, force }
    }

    pub fn process_trades(&self, transactions: &[Transaction]) -> Result<Vec<TaxReportEntry>> {
        let mut trades: Vec<&Transaction> = transactions
            .iter()
            .filter(|t| t.r#type == TransactionType::Trade)
            .collect();
        trades.sort_by_key(|t| t.date);

        let mut by_symbol: BTreeMap<&str, Vec<&Transaction>> = BTreeMap::new();
        for t in &trades {
            by_symbol.entry(&t.symbol).or_default().push(t);
        }

        let mut entries = Vec::new();
        for txns in by_symbol.values() {
            self.process_symbol(txns, &mut entries)?;
        }
        Ok(entries)
    }

    fn process_symbol(
        &self,
        txns: &[&Transaction],
        entries: &mut Vec<TaxReportEntry>,
    ) -> Result<()> {
        let mut inventory: VecDeque<Lot> = VecDeque::new();

        for txn in txns {
            if txn.quantity > Decimal::ZERO {
                inventory.push_back(Lot {
                    date: txn.date,
                    price: txn.price,
                    quantity: txn.quantity,
                    currency: txn.currency.clone(),
                });
                continue;
            }

            let mut remaining = txn.quantity.abs();
            let sale_date = txn.date;
            let sale_price = txn.price;
            let sale_currency = &txn.currency;

            while remaining > Decimal::ZERO {
                if inventory.is_empty() {
                    entries.push(self.create_entry(
                        &txn.symbol,
                        remaining,
                        sale_date,
                        sale_price,
                        sale_currency,
                        sale_date,
                        Decimal::ZERO,
                    )?);
                    break;
                }

                let lot = inventory.front_mut().unwrap();
                let lot_date = lot.date;
                let lot_price = lot.price;

                let matched = if lot.quantity <= remaining {
                    let m = lot.quantity;
                    inventory.pop_front();
                    m
                } else {
                    lot.quantity -= remaining;
                    remaining
                };

                entries.push(self.create_entry(
                    &txn.symbol,
                    matched,
                    sale_date,
                    sale_price,
                    sale_currency,
                    lot_date,
                    lot_price,
                )?);
                remaining -= matched;
            }
        }
        Ok(())
    }

    fn get_rate(&self, date: NaiveDate, currency: &Currency) -> Result<Decimal> {
        if *currency == Currency::RSD {
            return Ok(Decimal::ONE);
        }
        match self.nbs.get_rate(date, currency) {
            Ok(Some(rate)) => Ok(rate),
            Ok(None) if self.force => self.force_fallback_rate(date, currency),
            Ok(None) => Err(anyhow::anyhow!(
                "no NBS exchange rate found for {currency:?} on {date}"
            )),
            Err(e) if self.force => {
                warn!(%date, ?currency, error = %e, "rate fetch error in force mode, trying cache");
                self.force_fallback_rate(date, currency)
            }
            Err(e) => Err(e).context(format!(
                "failed to get NBS exchange rate for {currency:?} on {date}"
            )),
        }
    }

    fn force_fallback_rate(&self, date: NaiveDate, currency: &Currency) -> Result<Decimal> {
        if let Some(cached) =
            self.nbs
                .storage()
                .find_nearest_cached_rate(date, currency, FORCE_LOOKBACK_DAYS)
        {
            warn!(
                %date, ?currency, fallback_date = %cached.date, rate = %cached.rate,
                "force mode: using nearest cached rate"
            );
            Ok(cached.rate)
        } else {
            warn!(%date, ?currency, "force mode: no cached rate found at all");
            Err(anyhow::anyhow!(
                "no NBS exchange rate found for {currency:?} on {date} (even with force lookback)"
            ))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_entry(
        &self,
        ticker: &str,
        quantity: Decimal,
        sale_date: NaiveDate,
        sale_price: Decimal,
        sale_currency: &Currency,
        purchase_date: NaiveDate,
        purchase_price: Decimal,
    ) -> Result<TaxReportEntry> {
        let rate_sale = self.get_rate(sale_date, sale_currency)?;
        let rate_purchase = self.get_rate(purchase_date, sale_currency)?;

        let sale_value_rsd = round2(quantity * sale_price * rate_sale);
        let purchase_value_rsd = round2(quantity * purchase_price * rate_purchase);
        let capital_gain_rsd = round2(sale_value_rsd - purchase_value_rsd);
        let is_exempt = is_ten_year_exempt(purchase_date, sale_date);

        debug!(
            %ticker, %quantity, %sale_date, %purchase_date,
            %sale_value_rsd, %purchase_value_rsd, %capital_gain_rsd, %is_exempt,
            "created tax entry"
        );

        Ok(TaxReportEntry {
            ticker: ticker.to_string(),
            quantity,
            sale_date,
            sale_price,
            sale_exchange_rate: round4(rate_sale),
            sale_value_rsd,
            purchase_date,
            purchase_price,
            purchase_exchange_rate: round4(rate_purchase),
            purchase_value_rsd,
            capital_gain_rsd,
            is_tax_exempt: is_exempt,
        })
    }
}

fn is_ten_year_exempt(purchase_date: NaiveDate, sale_date: NaiveDate) -> bool {
    let target_year = purchase_date.year() + 10;
    let ten_years_later = purchase_date.with_year(target_year).unwrap_or_else(|| {
        NaiveDate::from_ymd_opt(target_year, purchase_date.month(), 28)
            .expect("Feb 28 always valid")
    });
    ten_years_later <= sale_date
}

fn round2(d: Decimal) -> Decimal {
    d.round_dp(2)
}

fn round4(d: Decimal) -> Decimal {
    d.round_dp(4)
}
