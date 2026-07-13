//! `timeglyph cal` — a forensics-grade calendar. This module is the pure data
//! builder: [`build_day`] computes the civil facts of a date (ISO week,
//! day-of-year, Julian Day Number, Modified JD, Unix midnight, weekday) with zero
//! I/O, so it is fully testable and serialisable. Timezone/leap overlays,
//! alternative calendars, the moon/season visual layer, rendering, and the CLI
//! live in sibling modules built on top of this.

use crate::ChronoError;
use jiff::civil::Date;

/// The civil facts of a single calendar day, serialised verbatim into the
/// machine (`--json`) view.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CalDay {
    /// ISO date `YYYY-MM-DD`.
    pub date: String,
    /// Lowercase English weekday name (`monday`..`sunday`).
    pub weekday: String,
    /// ISO-8601 week-numbering year (differs from the calendar year at Dec/Jan edges).
    pub iso_year: i16,
    /// ISO-8601 week number, 1–53.
    pub iso_week: i8,
    /// ISO-8601 weekday, 1 = Monday .. 7 = Sunday.
    pub iso_weekday: i8,
    /// Day of the year, 1–366.
    pub day_of_year: i16,
    /// Days in this year (365 or 366).
    pub days_in_year: i16,
    /// Julian Day Number (integer) of this date.
    pub jdn: i64,
    /// Modified Julian Day = `jdn − 2400001` (MJD epoch 1858-11-17).
    pub mjd: i64,
    /// Unix seconds at `00:00:00Z` of this date.
    pub unix_utc_midnight: i64,
}

/// Julian Day Number of the Unix epoch (1970-01-01).
const JDN_UNIX_EPOCH: i64 = 2_440_588;
/// `JDN − MJD` offset (the MJD epoch 1858-11-17 has JDN 2400001).
const MJD_OFFSET: i64 = 2_400_001;

/// Build the civil facts of `date` against a UTC reference. Pure; never panics.
///
/// # Errors
/// Returns [`ChronoError`] only if the date's UTC midnight is out of the
/// representable timestamp range (never for an ordinary calendar date).
pub fn build_day(date: Date) -> Result<CalDay, ChronoError> {
    let unix_utc_midnight = date
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        // cov:unreachable: UTC midnight of a valid civil date is always in range.
        .map_err(|e| ChronoError::Render(e.to_string()))?
        .timestamp()
        .as_second();

    let jdn = JDN_UNIX_EPOCH + unix_utc_midnight.div_euclid(86_400);
    let iwd = date.iso_week_date();
    let iso_weekday = date.weekday().to_monday_one_offset();
    let weekday = match iso_weekday {
        1 => "monday",
        2 => "tuesday",
        3 => "wednesday",
        4 => "thursday",
        5 => "friday",
        6 => "saturday",
        _ => "sunday",
    };

    Ok(CalDay {
        date: date.to_string(),
        weekday: weekday.to_string(),
        iso_year: iwd.year(),
        iso_week: iwd.week(),
        iso_weekday,
        day_of_year: date.day_of_year(),
        days_in_year: date.days_in_year(),
        jdn,
        mjd: jdn - MJD_OFFSET,
        unix_utc_midnight,
    })
}
