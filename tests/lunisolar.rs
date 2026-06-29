//! Chinese lunisolar calendar + 干支 four-pillar rendering (feature `lunisolar`).
//!
//! Unlike every other timeglyph conversion (a pure instant↔instant mapping), the
//! lunisolar/Ganzhi reading is **convention-relative**: it needs a reference
//! meridian (a [`RenderZone`]) to assign the astronomical instants to civil days,
//! and optionally a longitude for the hour pillar's true-solar-time correction.
//!
//! Validated against the independent `cnlunar` / `lunardate` Python oracles
//! (see the env-gated differential test). Convention-dependent pillars (year via
//! 立春, month via 節) are asserted only on mid-period dates where every common
//! convention agrees; the day/hour pillars and lunar date are convention-free.
#![cfg(feature = "lunisolar")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;
use timeglyph::lunisolar::{self, LunisolarReading};
use timeglyph::{format, PosixNs, RenderZone};

/// The instant at `unix_secs` seconds.
fn at(unix_secs: i64) -> PosixNs {
    format("unix").unwrap().decode_int(unix_secs).unwrap()
}

fn render(unix_secs: i64, tz: &str, lon: Option<f64>) -> LunisolarReading {
    let zone = RenderZone::parse(tz).unwrap();
    lunisolar::render(at(unix_secs), &zone, lon).unwrap()
}

#[test]
fn mid_year_date_all_conventions_agree() {
    // 2020-06-01T00:00:00Z == 2020-06-01 08:00 at +08:00 (China). Mid-year, so
    // 立春/CNY year conventions agree. 2020 has a leap 4th month (闰四月); this
    // instant falls in it. cnlunar: lunar 2020/4/10 leap, 庚子 辛巳 乙亥 庚辰.
    let r = render(1_590_969_600, "+08:00", None);
    assert_eq!((r.lunar_year, r.lunar_month, r.lunar_day), (2020, 4, 10));
    assert!(r.is_leap_month, "2020-06-01 is in the leap 4th month");
    assert_eq!(r.year_pillar, "庚子");
    assert_eq!(r.month_pillar, "辛巳");
    assert_eq!(r.day_pillar, "乙亥");
    assert_eq!(r.hour_pillar, "庚辰");
}

#[test]
fn day_and_hour_pillars_are_convention_independent() {
    // 2020-01-25T00:00:00Z == 2020-01-25 08:00 at +08:00 (Chinese New Year day).
    // The day pillar (丁卯) and hour pillar (08:00 → 辰時 → 甲辰) and the lunar
    // date (正月初一) are convention-free; the YEAR pillar is NOT asserted here
    // (立春 vs CNY boundary disagree on this date).
    let r = render(1_579_910_400, "+08:00", None);
    assert_eq!((r.lunar_year, r.lunar_month, r.lunar_day), (2020, 1, 1));
    assert!(!r.is_leap_month);
    assert_eq!(r.day_pillar, "丁卯");
    assert_eq!(r.hour_pillar, "甲辰");
}

#[test]
fn meridian_changes_the_date() {
    // The same absolute instant near a day boundary yields a DIFFERENT lunisolar
    // date under different meridians — the whole point of requiring a timezone.
    // 2020-01-24T20:00:00Z: at +08:00 it is 2020-01-25 04:00; at UTC it is still
    // 2020-01-24 20:00. Different civil day → different reading.
    let cst = render(1_579_896_000, "+08:00", None);
    let utc = render(1_579_896_000, "UTC", None);
    assert_ne!(
        (cst.lunar_day, cst.day_pillar.clone()),
        (utc.lunar_day, utc.day_pillar.clone()),
        "a near-midnight instant must read differently across meridians"
    );
}

#[test]
fn longitude_shifts_the_hour_pillar_via_true_solar_time() {
    // At meridian 120°E (+08:00) the 08:00 civil time is 辰時 (庚辰). An observer
    // far west at 75°E sees solar time ~3h earlier (05:00 → 卯時), so the hour
    // pillar's branch shifts to 卯. The lunar date is unchanged (it is civil).
    let civil = render(1_590_969_600, "+08:00", None);
    let solar = render(1_590_969_600, "+08:00", Some(75.0));
    assert_eq!(civil.hour_pillar, "庚辰");
    assert_ne!(solar.hour_pillar, civil.hour_pillar);
    assert!(
        solar.hour_pillar.contains('卯'),
        "75°E true-solar shifts 08:00 → ~05:00 → 卯時, got {}",
        solar.hour_pillar
    );
}

#[test]
fn solar_ephemeris_drives_terms() {
    // The stem-branch solar ephemeris: on 2020-06-01 the Sun's apparent ecliptic
    // longitude is ~71° (independently: 小滿 = 60° on ~May 20, 芒種 = 75° on
    // ~Jun 5, sun ~0.96°/day), so the current solar term is 小滿. λ is
    // instant-based (meridian-independent).
    let r = render(1_590_969_600, "+08:00", None);
    assert!(
        (70.0..72.0).contains(&r.solar_longitude_deg),
        "λ = {}, expected ~71°",
        r.solar_longitude_deg
    );
    assert_eq!(r.solar_term, "小滿");
}

#[test]
fn reading_surfaces_its_assumptions() {
    let r = render(1_590_969_600, "+08:00", None);
    assert!(!r.assumptions.is_empty());
    // The meridian and the year-pillar (立春) convention must be stated.
    let joined = r.assumptions.join(" ");
    assert!(joined.contains("立春") || joined.to_lowercase().contains("lichun"));
    assert!(joined.contains("+08") || joined.to_lowercase().contains("meridian"));
}

// --- CLI surface --------------------------------------------------------------

fn cli(args: &[&str]) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_timeglyph"))
        .args(args)
        .output()
        .unwrap();
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (s, out.status.code().unwrap_or(-1))
}

#[test]
fn cli_lunisolar_requires_a_timezone() {
    // The conversion is convention-relative: a missing --tz must fail loudly,
    // never silently assume one.
    let (out, code) = cli(&["lunisolar", "2020-06-01T00:00:00Z"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.to_lowercase().contains("timezone") || out.contains("--tz"),
        "{out}"
    );
}

#[test]
fn cli_lunisolar_renders_pillars() {
    let (out, code) = cli(&["lunisolar", "2020-06-01T00:00:00Z", "--tz", "+08:00"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("庚子") && out.contains("乙亥"), "{out}");
}

// --- Independent differential oracle (env-gated: cnlunar) ----------------------

/// True when python3 + cnlunar are importable.
fn cnlunar_available() -> bool {
    Command::new("python3")
        .args(["-c", "import cnlunar"])
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
fn differential_vs_cnlunar_oracle() {
    if !cnlunar_available() {
        eprintln!("skipping: python3 + cnlunar not available");
        return;
    }
    // Mid-period dates (away from CNY/立春/節 boundaries) where the year-pillar
    // convention does not bite — every field then agrees across conventions.
    // (unix_secs at 00:00Z, read at +08:00 = 08:00 China time.)
    for &unix in &[
        1_590_969_600i64,
        1_592_006_400,
        1_500_000_000,
        1_400_000_000,
    ] {
        let r = render(unix, "+08:00", None);
        // Oracle: feed cnlunar the SAME civil 08:00 China datetime.
        let off = jiff::Timestamp::from_nanosecond(at(unix).0)
            .unwrap()
            .to_zoned(jiff::tz::TimeZone::fixed(jiff::tz::offset(8)));
        let py = format!(
            "import datetime,cnlunar;a=cnlunar.Lunar(datetime.datetime({y},{mo},{d},{h},{mi}),godType='8char');print(a.lunarYear,a.lunarMonth,a.lunarDay,a.day8Char,a.twohour8Char,a.month8Char)",
            y = off.year(), mo = off.month(), d = off.day(), h = off.hour(), mi = off.minute(),
        );
        let out = Command::new("python3").args(["-c", &py]).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let f: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(r.lunar_year.to_string(), f[0], "year @ {unix}");
        assert_eq!(r.lunar_month.to_string(), f[1], "month @ {unix}");
        assert_eq!(r.lunar_day.to_string(), f[2], "day @ {unix}");
        assert_eq!(r.day_pillar, f[3], "day pillar @ {unix}");
        assert_eq!(r.hour_pillar, f[4], "hour pillar @ {unix}");
        assert_eq!(r.month_pillar, f[5], "month pillar @ {unix}");
    }
}

#[test]
fn out_of_range_instant_errors_not_panics() {
    // An instant outside jiff's civil range must degrade to a loud error, never
    // a panic (covers the from_nanosecond guard at the top of render()).
    let zone = RenderZone::parse("+08:00").unwrap();
    assert!(lunisolar::render(PosixNs(i128::MAX), &zone, None).is_err());
}
