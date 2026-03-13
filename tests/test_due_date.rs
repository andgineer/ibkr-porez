use chrono::NaiveDate;
use ibkr_porez::due_date::next_working_day;
use ibkr_porez::holidays::HolidayCalendar;

#[test]
fn test_next_working_day_simple() {
    let cal = HolidayCalendar::load_embedded();
    let base = NaiveDate::from_ymd_opt(2023, 6, 30).unwrap();
    let due = next_working_day(base, &cal);
    // June 30 + 30 = July 30, 2023. July 30 is a Sunday, so July 31 (Monday).
    assert_eq!(due, NaiveDate::from_ymd_opt(2023, 7, 31).unwrap());
}

#[test]
fn test_next_working_day_skips_holiday() {
    let mut cal = HolidayCalendar::empty();
    cal.set_fallback(false);
    let target = NaiveDate::from_ymd_opt(2023, 8, 14).unwrap();
    cal.add_year(2023, vec![target]);

    // base = July 15 => July 15 + 30 = Aug 14 => holiday => Aug 15
    let base = NaiveDate::from_ymd_opt(2023, 7, 15).unwrap();
    let due = next_working_day(base, &cal);
    assert_eq!(due, NaiveDate::from_ymd_opt(2023, 8, 15).unwrap());
}

#[test]
fn test_next_working_day_missing_year_fallback() {
    let cal = HolidayCalendar::empty();
    // Empty calendar, no fallback set, should use weekday-only logic
    let base = NaiveDate::from_ymd_opt(2023, 6, 30).unwrap();
    let due = next_working_day(base, &cal);
    // June 30 + 30 = July 30, 2023 (Sunday) => July 31 (Monday)
    assert_eq!(due, NaiveDate::from_ymd_opt(2023, 7, 31).unwrap());
}

#[test]
fn test_next_working_day_falls_on_weekday() {
    let cal = HolidayCalendar::load_embedded();
    // December 31, 2022 + 30 = January 30, 2023 (Monday). Jan 1 and Jan 2 are holidays
    // but +30 lands on the 30th which is a Monday and not a holiday
    let base = NaiveDate::from_ymd_opt(2022, 12, 31).unwrap();
    let due = next_working_day(base, &cal);
    assert_eq!(due, NaiveDate::from_ymd_opt(2023, 1, 30).unwrap());
}
