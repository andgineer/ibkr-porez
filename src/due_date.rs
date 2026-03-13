use chrono::{Duration, NaiveDate};
use tracing::debug;

use crate::holidays::HolidayCalendar;

/// Compute the filing due date: `base + 30 days`, then advance past
/// weekends and Serbian holidays. If holiday data is missing for the
/// target year, falls back to weekday-only logic.
#[must_use]
pub fn next_working_day(base: NaiveDate, holidays: &HolidayCalendar) -> NaiveDate {
    let mut date = base + Duration::days(30);
    loop {
        if HolidayCalendar::is_weekend(date) {
            date += Duration::days(1);
            continue;
        }
        match holidays.is_serbian_holiday(date) {
            Ok(true) => {
                date += Duration::days(1);
            }
            Ok(false) => break,
            Err(_) => {
                debug!(%date, "holiday data missing for due-date year, using weekday-only");
                break;
            }
        }
    }
    date
}
