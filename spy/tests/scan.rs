//! Tests for the cross-platform scan core (the testable half of the Humble
//! Object; the Win32/UIA shell is verified on Windows at runtime).
#![allow(clippy::unwrap_used)]

use timeglyph::{PosixNs, RenderZone, TzSemantics};
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
fn readings_are_ranked_in_window_datetimes_utc() {
    let r = scan::readings_for("1577836800", 5, &RenderZone::Utc);
    assert!(!r.is_empty());
    // The top reading for this value is Unix-seconds = 2020-01-01 UTC.
    assert!(
        r[0].format_id.contains("unix")
            && r[0].rendered.contains("2020-01-01")
            && r[0].rendered.ends_with('Z')
            && !r[0].local,
        "{r:?}"
    );
}

#[test]
fn display_zone_shifts_utc_anchored_readings() {
    // A chosen display zone re-expresses UTC-anchored readings with an explicit
    // offset: 2020-01-01T00:00Z → 2019-12-31T19:00−05:00.
    let r = scan::readings_for("1577836800", 5, &RenderZone::parse("-05:00").unwrap());
    let unix = r.iter().find(|x| x.format_id.contains("unix")).unwrap();
    assert!(
        unix.rendered.contains("2019-12-31") && unix.rendered.contains("-05:00") && !unix.local,
        "{unix:?}"
    );
}

#[test]
fn render_in_zone_respects_tz_semantics() {
    let inst = PosixNs(1_592_222_400_000_000_000); // 2020-06-15T12:00:00Z
    let native = inst.render(&RenderZone::Utc).unwrap();
    let east = RenderZone::parse("-05:00").unwrap();

    // UTC-anchored: the display zone shifts it and stamps the explicit offset.
    let (utc, _) = scan::render_in_zone(TzSemantics::Utc, inst, &native, &RenderZone::Utc);
    let (shifted, shifted_local) = scan::render_in_zone(TzSemantics::Utc, inst, &native, &east);
    assert!(utc.ends_with('Z'));
    assert!(
        shifted.contains("2020-06-15T07:00:00") && shifted.contains("-05:00") && !shifted_local,
        "{shifted}"
    );

    // Local-naive: NEVER shifted by a display zone (no UTC anchor); flagged local
    // and carrying no zone designator.
    let (a, a_local) =
        scan::render_in_zone(TzSemantics::LocalNaive, inst, &native, &RenderZone::Utc);
    let (b, _) = scan::render_in_zone(TzSemantics::LocalNaive, inst, &native, &east);
    assert!(a_local, "local-naive must be flagged local");
    assert_eq!(a, b, "local-naive must not be shifted by the display zone");
    assert!(
        a.contains("2020-06-15T12:00:00") && !a.contains('Z') && !a.contains("-05:00"),
        "{a}"
    );
}

#[test]
fn inspect_text_keeps_only_numbers_with_a_confident_reading() {
    let hits = scan::inspect_text(
        "the cookie value is 13390845530064940 (chrome)",
        5,
        &RenderZone::Utc,
    );
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].number, "13390845530064940");
    assert!(hits[0]
        .readings
        .iter()
        .any(|r| r.format_id.contains("webkit")));
    // A long-but-meaningless number yields no confident reading → dropped.
    assert!(scan::inspect_text("00000000000000000000", 5, &RenderZone::Utc).is_empty());
}

#[test]
fn string_datetimes_are_decoded() {
    // Rendered datetime STRINGS decode too, not just big integers.
    let r = scan::readings_for_string("2025-05-04T15:18:50Z", &RenderZone::Utc);
    assert!(r.iter().any(|x| x.format_id.contains("iso8601")), "{r:?}");
}

#[test]
fn inspect_text_catches_embedded_datetime_strings() {
    let hits = scan::inspect_text("modified: 2025-05-04T15:18:50Z (ok)", 5, &RenderZone::Utc);
    assert!(
        hits.iter().any(|h| h.number == "2025-05-04T15:18:50Z"
            && h.readings.iter().any(|r| r.format_id.contains("iso8601"))),
        "{hits:?}"
    );
}
