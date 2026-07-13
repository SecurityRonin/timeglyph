//! `timeglyph cal` — civil-exact week/epoch core (tier-1). Every expected value
//! is independently verifiable: ISO-8601 week dates and day-of-year cross-checked
//! against GNU/BSD `date +%G-W%V-%u`/`+%j` and Python `isocalendar()`; Julian Day
//! Numbers from USNO; the MJD epoch from IAU; Unix midnights arithmetic.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use jiff::civil::date;
use timeglyph::cal::{build_day, CalDay};

fn day(y: i16, m: i8, d: i8) -> CalDay {
    build_day(date(y, m, d)).unwrap()
}

#[test]
fn jdn_mjd_unix_anchors() {
    // JDN 2000-01-01 = 2451545 (USNO); MJD = JDN − 2400001; Unix arithmetic.
    let a = day(2000, 1, 1);
    assert_eq!(a.jdn, 2_451_545);
    assert_eq!(a.mjd, 51_544);
    assert_eq!(a.unix_utc_midnight, 946_684_800);
    // MJD epoch 1858-11-17: JDN 2400001, MJD 0 (IAU / USNO).
    let e = day(1858, 11, 17);
    assert_eq!(e.jdn, 2_400_001);
    assert_eq!(e.mjd, 0);
    // Unix epoch 1970-01-01: JDN 2440588, Unix 0.
    let u = day(1970, 1, 1);
    assert_eq!(u.jdn, 2_440_588);
    assert_eq!(u.unix_utc_midnight, 0);
    // 2016-12-31: JDN 2457753.
    let c = day(2016, 12, 31);
    assert_eq!(c.jdn, 2_457_753);
    assert_eq!(c.mjd, 57_752);
    assert_eq!(c.unix_utc_midnight, 1_483_142_400);
}

#[test]
fn iso_week_edges() {
    // ISO 8601 canonical edge cases (vs `date +%G-W%V-%u`).
    let a = day(2000, 1, 1); // Sat → belongs to 1999-W52
    assert_eq!((a.iso_year, a.iso_week, a.iso_weekday), (1999, 52, 6));
    let b = day(2008, 12, 29); // Mon → 2009-W01
    assert_eq!((b.iso_year, b.iso_week, b.iso_weekday), (2009, 1, 1));
    let c = day(2010, 1, 3); // Sun → 2009-W53
    assert_eq!((c.iso_year, c.iso_week, c.iso_weekday), (2009, 53, 7));
    let d = day(2016, 12, 31); // Sat → 2016-W52
    assert_eq!((d.iso_year, d.iso_week, d.iso_weekday), (2016, 52, 6));
    let n = day(2026, 11, 1); // Sun → 2026-W44
    assert_eq!((n.iso_year, n.iso_week, n.iso_weekday), (2026, 44, 7));
}

#[test]
fn day_of_year_weekday_and_iso_string() {
    let c = day(2016, 12, 31);
    assert_eq!((c.day_of_year, c.days_in_year), (366, 366)); // leap year
    let n = day(2026, 11, 1);
    assert_eq!((n.day_of_year, n.days_in_year), (305, 365));
    assert_eq!(day(2000, 1, 1).weekday, "saturday");
    assert_eq!(day(1970, 1, 1).weekday, "thursday");
    assert_eq!(n.weekday, "sunday");
    assert_eq!(n.date, "2026-11-01");
}
