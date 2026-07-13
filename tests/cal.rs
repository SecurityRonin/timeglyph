//! `timeglyph cal` — civil-exact week/epoch core (tier-1). Every expected value
//! is independently verifiable: ISO-8601 week dates and day-of-year cross-checked
//! against GNU/BSD `date +%G-W%V-%u`/`+%j` and Python `isocalendar()`; Julian Day
//! Numbers from USNO; the MJD epoch from IAU; Unix midnights arithmetic.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use jiff::civil::date;
use timeglyph::cal::{build_day, CalDay};
use timeglyph::RenderZone;

fn day(y: i16, m: i8, d: i8) -> CalDay {
    build_day(date(y, m, d), &RenderZone::Utc).unwrap()
}

fn zoned(y: i16, m: i8, d: i8, tz: &str) -> CalDay {
    let zone = RenderZone::parse(tz).unwrap();
    build_day(date(y, m, d), &zone).unwrap()
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
    // 2016-12-31: JDN 2457754 (= USNO JDN 2000-01-01 [2451545] + 6209 days).
    let c = day(2016, 12, 31);
    assert_eq!(c.jdn, 2_457_754);
    assert_eq!(c.mjd, 57_753);
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

// --- Cycle 2: leap-second days + GPS week (tier-1, hifitime IERS table) --------

#[cfg(feature = "leap")]
mod leapday {
    use timeglyph::leap::{gps_week, leap_seconds_on_utc_day};

    #[test]
    fn leap_second_days_from_iers_table() {
        // 2016-12-31 (unix midnight 1_483_142_400) and 2015-06-30 (1_435_622_400)
        // each carry an inserted leap second (IERS Bulletin C): cumulative TAI−UTC
        // rises by 1 across the UTC day (36→37, 35→36).
        assert_eq!(leap_seconds_on_utc_day(1_483_142_400), 1);
        assert_eq!(leap_seconds_on_utc_day(1_435_622_400), 1);
        // Ordinary days: no change.
        assert_eq!(leap_seconds_on_utc_day(1_483_056_000), 0); // 2016-12-30
        assert_eq!(leap_seconds_on_utc_day(1_483_228_800), 0); // 2017-01-01
    }

    #[test]
    fn gps_week_anchors() {
        assert_eq!(gps_week(315_964_800), 0); // 1980-01-06, GPS week 0
        assert_eq!(gps_week(1_554_595_200), 2048); // 2019-04-07 (post-rollover)
        assert_eq!(gps_week(1_793_491_200), 2443); // 2026-11-01
    }
}

// --- Cycle 3: TZ / DST overlay (tier-1 vs zdump / IANA tzdb) -------------------

#[test]
fn dst_fold_day_new_york() {
    // America/New_York 2026-11-01: fall-back FOLD at 06:00Z, -04:00 EDT -> -05:00
    // EST; the wall day is 25 h (90000 s). (zdump -v America/New_York)
    let d = zoned(2026, 11, 1, "America/New_York");
    assert_eq!(d.offset_start_seconds, -14400);
    assert_eq!(d.offset_end_seconds, -18000);
    assert_eq!(d.wall_day_seconds, 90_000);
    let t = d.dst_transition.expect("a transition on the fold day");
    assert_eq!(t.kind, "fold");
    assert_eq!(t.at_utc, "2026-11-01T06:00:00Z");
}

#[test]
fn dst_gap_day_new_york() {
    // 2026-03-08: spring-forward GAP at 07:00Z, -05:00 -> -04:00; wall day 23 h
    // (82800 s), 02:00-02:59 local never exists.
    let d = zoned(2026, 3, 8, "America/New_York");
    assert_eq!(d.offset_start_seconds, -18000);
    assert_eq!(d.offset_end_seconds, -14400);
    assert_eq!(d.wall_day_seconds, 82_800);
    assert_eq!(d.dst_transition.expect("gap").kind, "gap");
}

#[test]
fn dst_thirty_minute_fold_lord_howe() {
    // Australia/Lord_Howe 2026-04-05: 30-minute fall-back fold; wall day 24 h 30 m
    // (88200 s). (zdump -v Australia/Lord_Howe)
    let d = zoned(2026, 4, 5, "Australia/Lord_Howe");
    assert_eq!(d.wall_day_seconds, 88_200);
    assert_eq!(d.dst_transition.expect("fold").kind, "fold");
}

#[test]
fn ordinary_day_has_no_transition_and_full_wall_day() {
    let d = zoned(2026, 7, 1, "America/New_York");
    assert_eq!(d.wall_day_seconds, 86_400);
    assert!(d.dst_transition.is_none());
    assert_eq!(d.offset_start_seconds, -14400); // EDT
    // UTC day is always 86400 s except on a leap-second day.
    assert_eq!(day(2026, 7, 1).wall_day_seconds, 86_400);
}

#[cfg(feature = "leap")]
#[test]
fn leap_and_gps_fold_into_calday() {
    let c = day(2016, 12, 31);
    assert_eq!(c.leap_second, 1);
    assert_eq!(c.utc_day_seconds, 86_401);
    assert!(c.in_leap_smear_window);
    assert_eq!(c.gps_week, 1929);
    let n = day(2026, 7, 1);
    assert_eq!(n.leap_second, 0);
    assert_eq!(n.utc_day_seconds, 86_400);
    assert!(!n.in_leap_smear_window);
}

// --- Cycle 4: artifact ranges (registry epoch days + cited rollovers) ---------

fn has_artifact(d: &CalDay, kind: &str, name: &str) -> bool {
    d.artifacts.iter().any(|a| a.kind == kind && a.name == name)
}

#[test]
fn epoch_days_come_from_the_registry() {
    // Epoch instants are spec facts cited in forensicnomicon (tier-1).
    assert!(has_artifact(&day(1601, 1, 1), "epoch", "filetime"));
    assert!(has_artifact(&day(1601, 1, 1), "epoch", "webkit"));
    assert!(has_artifact(&day(1970, 1, 1), "epoch", "unix"));
    assert!(has_artifact(&day(1899, 12, 30), "epoch", "ole"));
    assert!(has_artifact(&day(1904, 1, 1), "epoch", "hfsplus"));
    assert!(has_artifact(&day(2001, 1, 1), "epoch", "cocoa"));
}

#[test]
fn rollovers_are_derived_from_structural_limits() {
    // 2038-01-19T03:14:07Z = i32::MAX seconds; 2106-02-07 = u32::MAX.
    assert!(has_artifact(&day(2038, 1, 19), "rollover", "unix_i32"));
    assert!(has_artifact(&day(2106, 2, 7), "rollover", "unix_u32"));
}

#[test]
fn ordinary_day_has_no_artifacts() {
    assert!(day(2026, 7, 1).artifacts.is_empty());
}

// --- Cycle 5a: Chinese/干支 overlay (reuse lunisolar; tier-1 vs cnlunar) -------

#[cfg(feature = "lunisolar")]
#[test]
fn chinese_overlay_matches_lunisolar() {
    // Any 2020 date after 立春 is year pillar 庚子; May is lunar month 4.
    let d = zoned(2020, 5, 31, "+08:00");
    let c = d.alt_chinese.expect("chinese overlay under lunisolar");
    assert_eq!(c.year_pillar, "庚子");
    assert_eq!(c.lunar_month, 4);
    // The overlay is exactly lunisolar::render at the day's noon in the zone.
    use timeglyph::{lunisolar, PosixNs, RenderZone};
    let zone = RenderZone::parse("+08:00").unwrap();
    let noon = jiff::civil::date(2020, 5, 31)
        .at(12, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::fixed(jiff::tz::offset(8)))
        .unwrap()
        .timestamp()
        .as_nanosecond();
    let r = lunisolar::render(PosixNs(noon), &zone, None).unwrap();
    assert_eq!(c.lunar_year, r.lunar_year);
    assert_eq!(c.lunar_day, r.lunar_day);
    assert_eq!(c.solar_term, r.solar_term);
}

// --- Cycle 5b: Hebrew + Islamic overlays (icu_calendar, feature=altcal) --------

#[cfg(feature = "altcal")]
#[test]
fn hebrew_and_islamic_overlays() {
    // 2007-09-13: Rosh Hashanah 5768 = 1 Tishrei 5768 (Hebrew); 1 Ramadan 1428
    // (Islamic tabular civil). Both independently verifiable (Hebcal / almanac).
    let d = day(2007, 9, 13);
    let h = d.alt_hebrew.expect("hebrew overlay");
    assert_eq!((h.year, h.month, h.day), (5768, 1, 1));
    let i = d.alt_islamic.expect("islamic overlay");
    assert_eq!((i.year, i.month, i.day), (1428, 9, 1));
}
