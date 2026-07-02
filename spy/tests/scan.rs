//! Tests for the cross-platform scan core (the testable half of the Humble
//! Object; the Win32/UIA shell is verified on Windows at runtime).
#![allow(clippy::unwrap_used)]

use timeglyph_spy::scan;

#[test]
fn extracts_only_long_numeric_runs() {
    // >= 8 consecutive digits are timestamp candidates; short runs (counts,
    // ids, years) are ignored so the overlay isn't noisy.
    let nums = scan::scan_numbers("created=1577836800 id=42 v2 13390845530064940!");
    assert_eq!(nums, vec!["1577836800", "13390845530064940"]);
    assert!(scan::scan_numbers("only short 42 and 2020 here").is_empty());
}

#[test]
fn readings_are_ranked_in_window_datetimes() {
    let r = scan::readings_for("1577836800", 5);
    assert!(!r.is_empty());
    // The top reading for this value is Unix-seconds = 2020-01-01.
    assert!(
        r[0].format_id.contains("unix") && r[0].rendered.contains("2020-01-01"),
        "{r:?}"
    );
}

#[test]
fn inspect_text_keeps_only_numbers_with_a_confident_reading() {
    let hits = scan::inspect_text("the cookie value is 13390845530064940 (chrome)", 5);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].number, "13390845530064940");
    assert!(hits[0]
        .readings
        .iter()
        .any(|r| r.format_id.contains("webkit")));
    // A long-but-meaningless number yields no confident reading → dropped.
    assert!(scan::inspect_text("00000000000000000000", 5).is_empty());
}
