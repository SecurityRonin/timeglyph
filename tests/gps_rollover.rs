//! GPS 1024-week rollover siblings. A legacy receiver stores the GPS week in 10
//! bits, so it aliases mod 1024 — the true week is `week + era·1024`. Without a
//! case/receiver date every era is a plausible reading (~20 years apart); with an
//! anchor the matching era is selected. Tier-1: the GPS epoch is 1980-01-06 and
//! the documented rollovers are 1999-08-22 and 2019-04-07 (IS-GPS-200 + advisories).
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(feature = "leap")]

use timeglyph::leap::gps_rollover_eras;

#[test]
fn week_zero_rollover_spans_the_documented_eras() {
    let eras = gps_rollover_eras(0, 0.0, None);
    let dates: Vec<&str> = eras.iter().map(|r| r.utc_rfc3339.as_str()).collect();
    assert!(
        dates.iter().any(|d| d.starts_with("1980-01")),
        "era 0 (epoch): {dates:?}"
    );
    assert!(
        dates.iter().any(|d| d.starts_with("1999-08")),
        "era 1 rollover: {dates:?}"
    );
    assert!(
        dates.iter().any(|d| d.starts_with("2019-04")),
        "era 2 rollover: {dates:?}"
    );
}

#[test]
fn an_anchor_selects_the_matching_era() {
    // A case around 2020 → the era-2 reading (2019-04) is the single selected one.
    let anchor_2020 = 1_577_836_800; // 2020-01-01 unix seconds
    let eras = gps_rollover_eras(0, 0.0, Some(anchor_2020));
    assert_eq!(eras.len(), 1, "an anchor selects one era, not all");
    assert!(
        eras[0].utc_rfc3339.starts_with("2019-04"),
        "the 2020 anchor picks the 2019 era: {:?}",
        eras[0].utc_rfc3339
    );
}

#[test]
fn each_era_reading_states_the_rollover_assumption() {
    let eras = gps_rollover_eras(100, 0.0, None);
    assert!(eras.iter().all(|r| {
        r.assumptions
            .iter()
            .any(|a| a.contains("mod 1024") && a.contains("era"))
    }));
}
