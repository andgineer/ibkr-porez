pub mod assess;
pub mod attach;
pub mod config;
pub mod export;
pub mod export_flex;
pub mod fetch;
pub mod import;
pub mod list;
pub mod output;
pub mod pay;
pub mod report;
pub mod revert;
pub mod show;
pub mod stat;
pub mod submit;
pub mod sync;
pub mod tables;

use anyhow::{Result, bail};
use chrono::{Datelike, Local, NaiveDate};
use rust_decimal::Decimal;

use ibkr_porez::config as app_config;
use ibkr_porez::holidays::HolidayCalendar;
use ibkr_porez::models::UserConfig;
use ibkr_porez::nbs::NBSClient;
use ibkr_porez::storage::Storage;

#[derive(Clone, clap::ValueEnum)]
pub enum StatusFilter {
    Draft,
    Submitted,
    Pending,
    Finalized,
}

impl StatusFilter {
    pub fn to_model(&self) -> ibkr_porez::models::DeclarationStatus {
        match self {
            Self::Draft => ibkr_porez::models::DeclarationStatus::Draft,
            Self::Submitted => ibkr_porez::models::DeclarationStatus::Submitted,
            Self::Pending => ibkr_porez::models::DeclarationStatus::Pending,
            Self::Finalized => ibkr_porez::models::DeclarationStatus::Finalized,
        }
    }
}

pub enum LibImportType {
    Auto,
    Csv,
    Flex,
}

impl From<super::ImportType> for LibImportType {
    fn from(t: super::ImportType) -> Self {
        match t {
            super::ImportType::Auto => Self::Auto,
            super::ImportType::Csv => Self::Csv,
            super::ImportType::Flex => Self::Flex,
        }
    }
}

pub enum LibReportType {
    Gains,
    Income,
}

impl From<super::ReportType> for LibReportType {
    fn from(t: super::ReportType) -> Self {
        match t {
            super::ReportType::Gains => Self::Gains,
            super::ReportType::Income => Self::Income,
        }
    }
}

fn load_config_or_exit() -> UserConfig {
    app_config::load_config()
}

fn make_storage(cfg: &UserConfig) -> Storage {
    Storage::with_config(cfg)
}

fn init_calendar(cfg: &UserConfig) -> HolidayCalendar {
    let mut cal = HolidayCalendar::load_embedded();
    let data_dir = app_config::get_effective_data_dir_path(cfg);
    cal.merge_file(&data_dir);

    let current_year = Local::now().year();
    if !cal.is_year_loaded(current_year) {
        eprintln!(
            "Warning: Holiday calendar data does not cover {current_year}. \
             Exchange rate lookback near holidays may be inaccurate.\n\
             Run `ibkr-porez sync` or update the app."
        );
    }
    cal
}

fn make_nbs<'a>(storage: &'a Storage, cal: &'a HolidayCalendar) -> NBSClient<'a> {
    NBSClient::new(storage, cal)
}

pub(crate) fn validate_non_negative_decimal(val: Decimal) -> Result<Decimal> {
    if val < Decimal::ZERO {
        bail!("value must be non-negative, got {val}");
    }
    Ok(val.round_dp(2))
}

/// Parse `--half` value in YYYY-H or YYYYH format (public for testing).
pub(crate) fn parse_half(s: &str) -> Result<(NaiveDate, NaiveDate)> {
    let s = s.trim();
    let (year_str, half_str) = if let Some(idx) = s.find('-') {
        (&s[..idx], &s[idx + 1..])
    } else if s.len() == 5 {
        (&s[..4], &s[4..])
    } else {
        bail!("invalid half format: {s} (expected YYYY-H or YYYYH)");
    };

    let year: i32 = year_str
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid year in half: {s}"))?;
    let half: u8 = half_str
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid half number in: {s}"))?;

    match half {
        1 => Ok((
            NaiveDate::from_ymd_opt(year, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(year, 6, 30).unwrap(),
        )),
        2 => Ok((
            NaiveDate::from_ymd_opt(year, 7, 1).unwrap(),
            NaiveDate::from_ymd_opt(year, 12, 31).unwrap(),
        )),
        _ => bail!("half must be 1 or 2, got {half}"),
    }
}

/// Resolve report period for gains reports (public for testing).
pub(crate) fn resolve_gains_period(
    half: Option<&str>,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> Result<(NaiveDate, NaiveDate)> {
    if let Some(h) = half {
        return parse_half(h);
    }
    match (start, end) {
        (Some(s), Some(e)) => {
            if s > e {
                bail!("start date {s} is after end date {e}");
            }
            Ok((s, e))
        }
        (Some(s), None) => Ok((s, s)),
        (None, Some(e)) => {
            let s = NaiveDate::from_ymd_opt(e.year(), 1, 1).unwrap();
            Ok((s, e))
        }
        (None, None) => {
            let today = Local::now().date_naive();
            let year = today.year();
            let month = today.month();
            if month < 7 {
                let prev = year - 1;
                Ok((
                    NaiveDate::from_ymd_opt(prev, 7, 1).unwrap(),
                    NaiveDate::from_ymd_opt(prev, 12, 31).unwrap(),
                ))
            } else {
                Ok((
                    NaiveDate::from_ymd_opt(year, 1, 1).unwrap(),
                    NaiveDate::from_ymd_opt(year, 6, 30).unwrap(),
                ))
            }
        }
    }
}

/// Resolve report period for income reports (public for testing).
pub(crate) fn resolve_income_period(
    half: Option<&str>,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> Result<(NaiveDate, NaiveDate)> {
    if let Some(h) = half {
        return parse_half(h);
    }
    match (start, end) {
        (Some(s), Some(e)) => {
            if s > e {
                bail!("start date {s} is after end date {e}");
            }
            Ok((s, e))
        }
        (Some(s), None) => Ok((s, s)),
        (None, Some(e)) => {
            let s = NaiveDate::from_ymd_opt(e.year(), e.month(), 1).unwrap();
            Ok((s, e))
        }
        (None, None) => {
            let today = Local::now().date_naive();
            let s = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            Ok((s, today))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn parse_half_yyyy_dash_h() {
        let (s, e) = parse_half("2026-1").unwrap();
        assert_eq!(s, d(2026, 1, 1));
        assert_eq!(e, d(2026, 6, 30));

        let (s, e) = parse_half("2025-2").unwrap();
        assert_eq!(s, d(2025, 7, 1));
        assert_eq!(e, d(2025, 12, 31));
    }

    #[test]
    fn parse_half_yyyyh() {
        let (s, e) = parse_half("20261").unwrap();
        assert_eq!(s, d(2026, 1, 1));
        assert_eq!(e, d(2026, 6, 30));

        let (s, e) = parse_half("20252").unwrap();
        assert_eq!(s, d(2025, 7, 1));
        assert_eq!(e, d(2025, 12, 31));
    }

    #[test]
    fn parse_half_invalid() {
        assert!(parse_half("2026-3").is_err());
        assert!(parse_half("2026-0").is_err());
        assert!(parse_half("abc").is_err());
        assert!(parse_half("").is_err());
        assert!(parse_half("2026").is_err());
    }

    #[test]
    fn parse_half_trims_whitespace() {
        let (s, e) = parse_half("  2026-1  ").unwrap();
        assert_eq!(s, d(2026, 1, 1));
        assert_eq!(e, d(2026, 6, 30));
    }

    #[test]
    fn gains_period_half_takes_precedence() {
        let (s, e) =
            resolve_gains_period(Some("2025-2"), Some(d(2020, 1, 1)), Some(d(2020, 12, 31)))
                .unwrap();
        assert_eq!(s, d(2025, 7, 1));
        assert_eq!(e, d(2025, 12, 31));
    }

    #[test]
    fn gains_period_explicit_dates() {
        let (s, e) = resolve_gains_period(None, Some(d(2025, 3, 1)), Some(d(2025, 6, 30))).unwrap();
        assert_eq!(s, d(2025, 3, 1));
        assert_eq!(e, d(2025, 6, 30));
    }

    #[test]
    fn gains_period_start_only_implies_end_equals_start() {
        let (s, e) = resolve_gains_period(None, Some(d(2025, 5, 15)), None).unwrap();
        assert_eq!(s, d(2025, 5, 15));
        assert_eq!(e, d(2025, 5, 15));
    }

    #[test]
    fn gains_period_end_only_uses_jan_1_of_year() {
        let (s, e) = resolve_gains_period(None, None, Some(d(2025, 5, 20))).unwrap();
        assert_eq!(s, d(2025, 1, 1));
        assert_eq!(e, d(2025, 5, 20));
    }

    #[test]
    fn gains_period_start_after_end_is_error() {
        let result = resolve_gains_period(None, Some(d(2025, 12, 1)), Some(d(2025, 1, 1)));
        assert!(result.is_err());
    }

    #[test]
    fn income_period_half_takes_precedence() {
        let (s, e) =
            resolve_income_period(Some("2026-1"), Some(d(2020, 1, 1)), Some(d(2020, 12, 31)))
                .unwrap();
        assert_eq!(s, d(2026, 1, 1));
        assert_eq!(e, d(2026, 6, 30));
    }

    #[test]
    fn income_period_explicit_dates() {
        let (s, e) =
            resolve_income_period(None, Some(d(2025, 3, 1)), Some(d(2025, 3, 31))).unwrap();
        assert_eq!(s, d(2025, 3, 1));
        assert_eq!(e, d(2025, 3, 31));
    }

    #[test]
    fn income_period_start_only_implies_end_equals_start() {
        let (s, e) = resolve_income_period(None, Some(d(2025, 7, 10)), None).unwrap();
        assert_eq!(s, d(2025, 7, 10));
        assert_eq!(e, d(2025, 7, 10));
    }

    #[test]
    fn income_period_default_is_current_month() {
        let (s, e) = resolve_income_period(None, None, None).unwrap();
        let today = Local::now().date_naive();
        assert_eq!(s, d(today.year(), today.month(), 1));
        assert_eq!(e, today);
    }

    #[test]
    fn validate_non_negative_rejects_negative() {
        assert!(validate_non_negative_decimal(dec!(-1.00)).is_err());
        assert!(validate_non_negative_decimal(dec!(-0.01)).is_err());
    }

    #[test]
    fn validate_non_negative_rounds_to_2dp() {
        let result = validate_non_negative_decimal(dec!(123.456)).unwrap();
        assert_eq!(result, dec!(123.46));
    }

    #[test]
    fn validate_non_negative_accepts_zero() {
        let result = validate_non_negative_decimal(dec!(0)).unwrap();
        assert_eq!(result, dec!(0.00));
    }
}
