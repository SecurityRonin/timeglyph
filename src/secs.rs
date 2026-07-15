//! Whole-second convenience over the [`PosixNs`] spine, for callers that store a
//! plain `i64` Unix-seconds timestamp (filesystem `FsMeta`, bodyfile rows) rather
//! than a full nanosecond instant. Thin wrappers — the epoch math still lives in
//! the canonical converters; this only drops sub-second precision and the two-
//! word/`PosixNs` ceremony for the common "field → seconds" case.

use crate::PosixNs;

/// Unix seconds from a whole 64-bit Windows `FILETIME` (100 ns ticks since 1601),
/// or `None` when it is out of the decodable range. Convenience over
/// [`crate::compose::filetime_hilo`] for callers holding the value as one `u64`.
#[must_use]
pub fn filetime(ft: u64) -> Option<i64> {
    let low = (ft & 0xFFFF_FFFF) as u32;
    let high = (ft >> 32) as u32;
    crate::compose::filetime_hilo(low, high)
        .ok()
        .map(PosixNs::unix_seconds)
}

/// Unix seconds for a civil UTC date/time given as broken-down fields — the
/// common shape of a filesystem/archive timestamp (ext-style epoch dates, ZIP
/// MS-DOS dates already normalised to y/m/d h:m:s). `None` for a field
/// combination outside the representable range (never panics).
#[must_use]
pub fn civil(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Option<i64> {
    // Fallible constructors: `jiff::civil::datetime` PANICS on an invalid field
    // combination, but these fields come from untrusted on-disk data, so build
    // via `Date::new`/`Time::new` which return `Err` (→ `None`) instead.
    let date = jiff::civil::Date::new(
        i16::try_from(year).ok()?,
        i8::try_from(month).ok()?,
        i8::try_from(day).ok()?,
    )
    .ok()?;
    let time = jiff::civil::Time::new(
        i8::try_from(hour).ok()?,
        i8::try_from(minute).ok()?,
        i8::try_from(second).ok()?,
        0,
    )
    .ok()?;
    date.to_datetime(time)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .ok()
        .map(|z| z.timestamp().as_second())
}

#[cfg(test)]
mod tests {
    use super::{civil, filetime};
    use crate::PosixNs;

    #[test]
    fn posix_ns_to_unix_seconds_floors() {
        assert_eq!(PosixNs(0).unix_seconds(), 0);
        assert_eq!(PosixNs(1_000_000_000).unix_seconds(), 1);
        assert_eq!(PosixNs(1_500_000_000).unix_seconds(), 1); // truncates sub-second
        assert_eq!(PosixNs(-1_000_000_000).unix_seconds(), -1);
        assert_eq!(PosixNs(-500_000_000).unix_seconds(), -1); // floor, not toward-zero
    }

    #[test]
    fn filetime_epoch_offset_is_unix_zero() {
        // FILETIME 116_444_736_000_000_000 == 1970-01-01 00:00:00 UTC.
        assert_eq!(filetime(116_444_736_000_000_000), Some(0));
    }

    #[test]
    fn filetime_known_value() {
        // 2000-01-01 00:00:00 UTC == unix 946_684_800.
        // ticks = (946_684_800 + 11_644_473_600) * 10_000_000
        assert_eq!(filetime(125_911_584_000_000_000), Some(946_684_800));
    }

    #[test]
    fn filetime_pre_1970_still_decodes_negative() {
        // FILETIME 0 == 1601-01-01, well before Unix epoch → negative seconds.
        assert_eq!(filetime(0), Some(-11_644_473_600));
    }

    #[test]
    fn civil_known_dates() {
        assert_eq!(civil(1970, 1, 1, 0, 0, 0), Some(0));
        assert_eq!(civil(2000, 1, 1, 0, 0, 0), Some(946_684_800));
        assert_eq!(civil(2021, 3, 1, 12, 30, 15), Some(1_614_601_815));
    }

    #[test]
    fn civil_out_of_range_is_none_not_panic() {
        assert_eq!(civil(2021, 13, 1, 0, 0, 0), None); // month 13
        assert_eq!(civil(2021, 2, 30, 0, 0, 0), None); // Feb 30
        assert_eq!(civil(i32::MAX, 1, 1, 0, 0, 0), None); // year out of i16
    }
}
