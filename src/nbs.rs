use anyhow::Result;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use tracing::debug;

use crate::holidays::HolidayCalendar;
use crate::models::{Currency, ExchangeRate};
use crate::storage::Storage;

const DEFAULT_BASE_URL: &str = "https://kurs.resenje.org/api/v1";
const MAX_LOOKBACK_DAYS: u32 = 10;
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

pub struct NBSClient<'a> {
    storage: &'a Storage,
    holidays: &'a HolidayCalendar,
    base_url: String,
    http: reqwest::blocking::Client,
}

impl<'a> NBSClient<'a> {
    #[must_use]
    pub fn new(storage: &'a Storage, holidays: &'a HolidayCalendar) -> Self {
        Self {
            storage,
            holidays,
            base_url: DEFAULT_BASE_URL.to_string(),
            http: build_http_client(),
        }
    }

    #[must_use]
    pub fn with_base_url(
        storage: &'a Storage,
        holidays: &'a HolidayCalendar,
        base_url: &str,
    ) -> Self {
        Self {
            storage,
            holidays,
            base_url: base_url.to_string(),
            http: build_http_client(),
        }
    }

    #[must_use]
    pub fn storage(&self) -> &Storage {
        self.storage
    }

    /// Get the NBS middle exchange rate for a currency on a given date.
    ///
    /// Returns `Ok(None)` if no rate could be found within the 10-day lookback window.
    /// Automatically handles weekends, Serbian holidays, caching, and retries.
    ///
    /// Returns `Err` with `HolidayError::MissingYear` if holiday data for the
    /// needed year is not available (unless fallback is enabled on the calendar).
    pub fn get_rate(&self, date: NaiveDate, currency: &Currency) -> Result<Option<Decimal>> {
        if *currency == Currency::RSD {
            return Ok(Some(Decimal::ONE));
        }

        let mut target = date;
        for _ in 0..MAX_LOOKBACK_DAYS {
            if let Some(cached) = self.storage.get_exchange_rate(target, currency) {
                debug!(%target, ?currency, rate = %cached.rate, "cache hit");
                if target != date {
                    self.storage.save_exchange_rate(&ExchangeRate {
                        date,
                        currency: currency.clone(),
                        rate: cached.rate,
                    })?;
                }
                return Ok(Some(cached.rate));
            }

            if HolidayCalendar::is_weekend(target) || self.holidays.is_serbian_holiday(target)? {
                debug!(%target, "skipping weekend/holiday");
                target -= chrono::Duration::days(1);
                continue;
            }

            match self.fetch_rate(target, currency) {
                Ok(Some(rate)) => {
                    debug!(%target, ?currency, %rate, "fetched rate");
                    self.storage.save_exchange_rate(&ExchangeRate {
                        date: target,
                        currency: currency.clone(),
                        rate,
                    })?;
                    if target != date {
                        self.storage.save_exchange_rate(&ExchangeRate {
                            date,
                            currency: currency.clone(),
                            rate,
                        })?;
                    }
                    return Ok(Some(rate));
                }
                Ok(None) => {
                    debug!(%target, ?currency, "no rate available");
                    target -= chrono::Duration::days(1);
                }
                Err(e) => {
                    debug!(%target, ?currency, error = %e, "fetch failed, looking back");
                    target -= chrono::Duration::days(1);
                }
            }
        }

        debug!(%date, ?currency, "no rate found within lookback window");
        Ok(None)
    }

    fn fetch_rate(&self, date: NaiveDate, currency: &Currency) -> Result<Option<Decimal>> {
        let url = format!(
            "{}/currencies/{}/rates/{}",
            self.base_url,
            currency.as_lowercase(),
            date.format("%Y-%m-%d")
        );

        let mut last_err = None;
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                std::thread::sleep(RETRY_DELAY);
            }
            match self.http.get(&url).send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_client_error() {
                        return Ok(None);
                    }
                    if status.is_server_error() {
                        last_err = Some(anyhow::anyhow!("HTTP {status}"));
                        continue;
                    }
                    let body: NbsRateResponse = resp.json()?;
                    if let Some(rate_f) = body.exchange_middle {
                        let rate = Decimal::from_str(&rate_f.to_string())?;
                        return Ok(Some(rate));
                    }
                    return Ok(None);
                }
                Err(e) => {
                    last_err = Some(e.into());
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch rate failed")))
    }
}

fn build_http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("TLS backend unavailable")
}

#[derive(Debug, Deserialize)]
struct NbsRateResponse {
    exchange_middle: Option<f64>,
}
