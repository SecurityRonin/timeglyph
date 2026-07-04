//! Holiday lookup, validated against python-holidays (the generating oracle).
//! These expected values are the reference project's own output for the given
//! country/date — an independent third party authored both the rule and the
//! answer (Evidence-Based Rigor, tier 1) — so they pin real behaviour, not a
//! fixture we invented.
#![cfg(feature = "holiday")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use jiff::civil::date;
use timeglyph::{holiday, RenderZone};

#[test]
fn known_fixed_holiday() {
    // US Independence Day is a fixed 07-04 in the reference data.
    assert_eq!(
        holiday::lookup("US", date(2020, 7, 4)),
        Some("Independence Day".to_string())
    );
    assert_eq!(
        holiday::lookup("US", date(2020, 12, 25)),
        Some("Christmas Day".to_string())
    );
}

#[test]
fn ordinary_day_is_none() {
    // A plain Monday, no holiday in the reference data.
    assert_eq!(holiday::lookup("US", date(2020, 3, 16)), None);
    // 2020-07-04 fell on a Saturday; the 05th (Sunday) is not itself a holiday
    // (the observed shift lands on Friday the 3rd).
    assert_eq!(holiday::lookup("US", date(2020, 7, 5)), None);
}

#[test]
fn lunar_holiday_native_locale() {
    // Chinese New Year 2020 fell on 01-25; the reference emits the native name.
    let cny = holiday::lookup("CN", date(2020, 1, 25)).expect("CN 2020-01-25 is a holiday");
    assert!(cny.contains('\u{6625}'), "expected 春节, got {cny:?}"); // 春 (Spring)
}

#[test]
fn unknown_country_is_none() {
    assert_eq!(holiday::lookup("ZZ", date(2020, 1, 1)), None);
}

#[test]
fn case_insensitive_country() {
    assert_eq!(
        holiday::lookup("us", date(2020, 7, 4)),
        Some("Independence Day".to_string())
    );
}

#[test]
fn zone_maps_to_country() {
    // IANA zone → ISO country, from the tz database's zone.tab.
    assert_eq!(holiday::country_for_zone("Asia/Shanghai"), Some("CN"));
    assert_eq!(holiday::country_for_zone("America/New_York"), Some("US"));
    assert_eq!(holiday::country_for_zone("Europe/London"), Some("GB"));
    // No single country → no annotation.
    assert_eq!(holiday::country_for_zone("Etc/UTC"), None);
    assert_eq!(holiday::country_for_zone("Nowhere/Nope"), None);
}

#[test]
fn holiday_in_named_zone() {
    let sh = RenderZone::Named(jiff::tz::TimeZone::get("Asia/Shanghai").unwrap());
    let cny = holiday::in_zone(&sh, date(2020, 1, 25)).expect("CNY in Asia/Shanghai");
    assert!(cny.contains('\u{6625}'), "expected 春节, got {cny:?}");
    // UTC / fixed offsets have no single country → no annotation.
    assert_eq!(holiday::in_zone(&RenderZone::Utc, date(2020, 1, 25)), None);
}

#[test]
fn dataset_loaded() {
    // Fail loud in CI if the embedded blob is missing/truncated/unparseable:
    // the generator ships ~248 countries, so a near-zero count is a regression.
    assert!(
        holiday::supported_country_count() > 200,
        "holiday dataset looks empty/truncated: {} countries",
        holiday::supported_country_count()
    );
}
