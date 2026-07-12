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
