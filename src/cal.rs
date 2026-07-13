//! `timeglyph cal` — a forensics-grade calendar. This module is the pure data
//! builder: [`build_day`] computes the civil + timezone facts of a date (ISO
//! week, day-of-year, Julian Day Number, Modified JD, Unix midnight, weekday,
//! per-day UTC offset, DST fold/gap, and — behind the `leap` feature — leap-second
//! days and GPS week) with zero I/O, so it is fully testable and serialisable.
//! Alternative calendars, the moon/season visual layer, rendering, and the CLI
//! live in sibling modules built on top of this.

use crate::{ChronoError, RenderZone};
use jiff::civil::Date;

/// A DST transition occurring within a calendar day.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DstTransition {
    /// `"gap"` (spring-forward, wall times skipped) or `"fold"` (fall-back, wall
    /// times repeated).
    pub kind: String,
    /// The UTC instant of the transition, RFC 3339.
    pub at_utc: String,
}

/// The civil + timezone facts of a single calendar day, serialised verbatim into
/// the machine (`--json`) view.
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
    /// UTC offset (seconds) in effect at the start of the day in the render zone.
    pub offset_start_seconds: i32,
    /// UTC offset (seconds) in effect at the end of the day (i.e. at next midnight).
    pub offset_end_seconds: i32,
    /// Elapsed real seconds of the wall-clock day in the render zone (86400 on an
    /// ordinary day; 23 h / 25 h / etc. across a DST transition).
    pub wall_day_seconds: i64,
    /// The DST transition within the day, if any.
    pub dst_transition: Option<DstTransition>,
    /// Leap seconds inserted (`+1`) / deleted (`-1`) during this UTC day.
    #[cfg(feature = "leap")]
    pub leap_second: i8,
    /// Length of the UTC day in seconds (`86400 + leap_second`).
    #[cfg(feature = "leap")]
    pub utc_day_seconds: i64,
    /// `true` if this UTC day is within ±12 h of a leap second (cloud-smear window).
    #[cfg(feature = "leap")]
    pub in_leap_smear_window: bool,
    /// GPS week number containing this day's 00:00 UTC instant.
    #[cfg(feature = "leap")]
    pub gps_week: i64,
}

/// Julian Day Number of the Unix epoch (1970-01-01).
const JDN_UNIX_EPOCH: i64 = 2_440_588;
/// `JDN − MJD` offset (the MJD epoch 1858-11-17 has JDN 2400001).
const MJD_OFFSET: i64 = 2_400_001;

/// The jiff time zone for a render zone.
fn zone_to_tz(zone: &RenderZone) -> jiff::tz::TimeZone {
    match zone {
        RenderZone::Utc => jiff::tz::TimeZone::UTC,
        RenderZone::Fixed(offset) => offset.to_time_zone(),
        RenderZone::Named(tz) => tz.clone(),
    }
}

/// The first instant of `date` in `tz` (handles a midnight DST gap by moving to
/// the first valid instant, per jiff's compatible disambiguation).
fn first_instant(date: Date, tz: &jiff::tz::TimeZone) -> Result<jiff::Timestamp, ChronoError> {
    date.at(0, 0, 0, 0)
        .to_zoned(tz.clone())
        .map(|z| z.timestamp())
        .map_err(|e| ChronoError::Render(e.to_string()))
}

/// Build the civil + timezone facts of `date` in `zone`. Pure; never panics.
///
/// # Errors
/// Returns [`ChronoError`] only if the date is at the edge of the representable
/// range (never for an ordinary calendar date).
pub fn build_day(date: Date, zone: &RenderZone) -> Result<CalDay, ChronoError> {
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

    // Timezone overlay.
    let tz = zone_to_tz(zone);
    let tomorrow = date
        .tomorrow()
        // cov:unreachable: only fails at the maximum representable civil date.
        .map_err(|e| ChronoError::Render(e.to_string()))?;
    let start = first_instant(date, &tz)?;
    let next = first_instant(tomorrow, &tz)?;
    let offset_start_seconds = tz.to_offset(start).seconds();
    let offset_end_seconds = tz.to_offset(next).seconds();
    let wall_day_seconds = next.as_second() - start.as_second();
    let dst_transition = tz.following(start).next().and_then(|t| {
        (t.timestamp() < next).then(|| {
            let after = t.offset().seconds();
            DstTransition {
                kind: if after > offset_start_seconds {
                    "gap"
                } else {
                    "fold"
                }
                .to_string(),
                at_utc: t.timestamp().to_string(),
            }
        })
    });

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
        offset_start_seconds,
        offset_end_seconds,
        wall_day_seconds,
        dst_transition,
        #[cfg(feature = "leap")]
        leap_second: crate::leap::leap_seconds_on_utc_day(unix_utc_midnight),
        #[cfg(feature = "leap")]
        utc_day_seconds: 86_400
            + i64::from(crate::leap::leap_seconds_on_utc_day(unix_utc_midnight)),
        // Centre the ±12 h smear probe on the day's noon, so the day's own
        // window is [midnight, next midnight] — which brackets a real leap second
        // (always at 23:59:60), flagging the leap day itself.
        #[cfg(feature = "leap")]
        in_leap_smear_window: crate::leap::within_leap_smear_window(unix_utc_midnight + 43_200),
        #[cfg(feature = "leap")]
        gps_week: crate::leap::gps_week(unix_utc_midnight),
    })
}
