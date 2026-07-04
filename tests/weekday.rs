//! The weekday shown after each reading is derived from the *displayed* ISO date
//! (so it always matches what the row shows, regardless of zone/format).
#![allow(clippy::unwrap_used)]

use timeglyph::scan;

#[test]
fn weekday_from_the_displayed_iso_date() {
    // 2020-01-01 is a Wednesday; 2021-07-01 is a Thursday.
    assert_eq!(scan::weekday("2020-01-01T00:00:00Z"), Some("Wednesday"));
    assert_eq!(scan::weekday("2021-07-01T12:00:00+08:00"), Some("Thursday"));
}

#[test]
fn weekday_is_none_for_a_non_date() {
    assert_eq!(scan::weekday("local text"), None);
    assert_eq!(scan::weekday("short"), None);
}
