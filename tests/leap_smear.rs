//! Leap-second smear window. Cloud clocks (Google/AWS) "smear" a leap second
//! across a ~24h window, so a value within ±12h of a leap second may be off by up
//! to a second. Detected authoritatively from hifitime's IERS table — a leap
//! second is in the window iff the cumulative offset changes across it — never a
//! hardcoded date.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(feature = "leap")]

use timeglyph::leap::within_leap_smear_window;

#[test]
fn instant_near_the_2016_leap_second_is_in_the_smear_window() {
    // The 2016-12-31 leap second: transition at 2017-01-01T00:00:00Z = unix 1_483_228_800.
    assert!(
        within_leap_smear_window(1_483_228_800),
        "±12h of the 2016 leap second is a smear window"
    );
}

#[test]
fn an_ordinary_instant_is_not_in_a_smear_window() {
    // 2020-06-15: no leap second within ±12h.
    assert!(!within_leap_smear_window(1_592_179_200));
}

#[test]
fn a_reading_near_a_leap_second_carries_the_smear_note() {
    // unix 1_483_228_800 = 2017-01-01, just after the 2016 leap second.
    let cands = timeglyph::interpret::interpret_int(1_483_228_800);
    let unix = cands
        .iter()
        .find(|c| c.format_id == "unix")
        .expect("a unix reading");
    assert!(
        unix.assumptions
            .iter()
            .any(|a| a.to_lowercase().contains("leap second") && a.to_lowercase().contains("smear")),
        "the unix reading near a leap second carries the smear note: {:?}",
        unix.assumptions
    );
}
