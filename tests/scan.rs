//! Tests for the cross-platform scan core (the testable half of the Humble
//! Object; the Win32/UIA shell is verified on Windows at runtime).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::scan;
use timeglyph::{DateStyle, PosixNs, RenderZone, TzSemantics};

#[test]
fn extracts_only_long_numeric_runs() {
    // >= 8 consecutive digits are timestamp candidates; short runs (counts,
    // ids, years) are ignored so the overlay isn't noisy.
    let nums = scan::scan_numbers("created=1577836800 id=42 v2 13390845530064940!");
    assert_eq!(nums, vec!["1577836800", "13390845530064940"]);
    assert!(scan::scan_numbers("only short 42 and 2020 here").is_empty());
}

#[test]
fn captures_fractional_float_as_one_token() {
    // A CFAbsoluteTime like WhatsApp-iOS ZMESSAGEDATE is a double; the scanner
    // must keep the fraction as part of the token, not stop at the '.'.
    let nums = scan::scan_numbers("ZMESSAGEDATE=606940977.71577 row");
    assert_eq!(nums, vec!["606940977.71577"], "{nums:?}");
}

#[test]
fn trailing_dot_and_dotted_shorts_are_not_floats() {
    // A trailing dot in prose is punctuation, not a fraction.
    assert_eq!(
        scan::scan_numbers("the value is 1577836800. done"),
        vec!["1577836800"]
    );
    // Dotted short runs (IPs, version strings) stay below the digit floor and
    // must not be glued into one long token.
    assert!(scan::scan_numbers("ip 192.168.11.100 v1.2.3").is_empty());
}

#[test]
fn readings_for_fractional_float_yields_cocoa_float() {
    let r = scan::readings_for("606940977.71577", 5, &RenderZone::Utc);
    assert!(
        r.iter().any(
            |x| x.format_id == "cocoa_float" && x.rendered.starts_with("2020-03-26T18:42:57.7")
        ),
        "{r:?}"
    );
}

#[test]
fn inspect_text_surfaces_cocoa_float_for_a_fractional_value() {
    let hits = scan::inspect_text("msg 606940977.71577 sent", 5, &RenderZone::Utc);
    let hit = hits
        .iter()
        .find(|h| h.number == "606940977.71577")
        .expect("fractional value scanned as one token");
    assert!(
        hit.readings.iter().any(|r| r.format_id == "cocoa_float"),
        "{:?}",
        hit.readings
    );
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
    let (utc, _) = scan::render_in_zone(
        TzSemantics::Utc,
        inst,
        &native,
        &RenderZone::Utc,
        DateStyle::Iso8601,
    );
    let (shifted, shifted_local) =
        scan::render_in_zone(TzSemantics::Utc, inst, &native, &east, DateStyle::Iso8601);
    assert!(utc.ends_with('Z'));
    assert!(
        shifted.contains("2020-06-15T07:00:00") && shifted.contains("-05:00") && !shifted_local,
        "{shifted}"
    );

    // Local-naive: NEVER shifted by a display zone (no UTC anchor); flagged local
    // and carrying no zone designator.
    let (a, a_local) = scan::render_in_zone(
        TzSemantics::LocalNaive,
        inst,
        &native,
        &RenderZone::Utc,
        DateStyle::Iso8601,
    );
    let (b, _) = scan::render_in_zone(
        TzSemantics::LocalNaive,
        inst,
        &native,
        &east,
        DateStyle::Iso8601,
    );
    assert!(a_local, "local-naive must be flagged local");
    assert_eq!(a, b, "local-naive must not be shifted by the display zone");
    assert!(
        a.contains("2020-06-15T12:00:00") && !a.contains('Z') && !a.contains("-05:00"),
        "{a}"
    );
}

#[test]
fn local_naive_honors_display_style_without_a_fabricated_offset() {
    // A naive wall-clock (FAT/exFAT/DOS) must render in the chosen STYLE — the
    // style is text formatting, separate from the zone shift — but carry NO
    // offset/zone designator, since a naive value has none (adding +0000 would
    // fabricate a UTC claim).
    let inst = PosixNs(1_592_222_400_000_000_000); // 2020-06-15T12:00:00 wall-clock
    let native = inst.render(&RenderZone::Utc).unwrap();

    let (rfc, is_local) = scan::render_in_zone(
        TzSemantics::LocalNaive,
        inst,
        &native,
        &RenderZone::Utc,
        DateStyle::Rfc2822,
    );
    assert!(is_local, "still flagged local");
    assert_eq!(rfc, "Mon, 15 Jun 2020 12:00:00", "RFC style, no offset");
    assert!(!rfc.contains("+0000") && !rfc.contains('Z'), "{rfc}");

    let (us, _) = scan::render_in_zone(
        TzSemantics::LocalNaive,
        inst,
        &native,
        &RenderZone::Utc,
        DateStyle::UsStyle,
    );
    assert_eq!(us, "06/15/2020 12:00:00 PM", "US style, no zone abbrev");

    // Regression: a UTC-anchored value in RFC style DOES keep its explicit offset.
    let (utc_rfc, _) = scan::render_in_zone(
        TzSemantics::Utc,
        inst,
        &native,
        &RenderZone::Utc,
        DateStyle::Rfc2822,
    );
    assert!(utc_rfc.contains("+0000"), "UTC RFC keeps offset: {utc_rfc}");
}

#[test]
fn render_in_zone_falls_back_to_native_when_out_of_range() {
    // A PosixNs beyond jiff's civil range cannot render, so the UTC branch falls
    // back to the format's own (native) string rather than dropping the value.
    let (rendered, local) = scan::render_in_zone(
        TzSemantics::Utc,
        PosixNs(i128::MAX),
        "9999-12-31T23:59:59Z",
        &RenderZone::Utc,
        DateStyle::Iso8601,
    );
    assert_eq!(rendered, "9999-12-31T23:59:59Z");
    assert!(!local);
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

#[test]
fn inspect_text_decodes_0x_prefixed_hex_token() {
    // A `0x`-prefixed hex token is decoded under the hex byte layouts, so `scan`
    // and the lens pick up raw hex — here the LE seconds reading is Unix time.
    let hits = scan::inspect_text("field = 0xa45a597a here", 8, &RenderZone::Utc);
    let hit = hits
        .iter()
        .find(|h| h.number == "0xa45a597a")
        .unwrap_or_else(|| panic!("0x token not decoded: {hits:?}"));
    assert!(
        hit.readings.iter().any(|r| r.format_id.contains("unix")),
        "{hit:?}"
    );
}

#[test]
fn inspect_text_decodes_bare_hex_with_letters() {
    // A bare hex run with a-f letters and >= 8 chars (>= 4 bytes, even length) is
    // decoded as raw hex bytes.
    let hits = scan::inspect_text("val 0060947C58B2D501 x", 5, &RenderZone::Utc);
    assert!(
        hits.iter().any(|h| h.number == "0060947C58B2D501"),
        "expected a hex reading: {hits:?}"
    );
}

#[test]
fn inspect_text_decimal_run_still_decodes() {
    // The decimal integer path is unaffected by hex-token detection.
    let hits = scan::inspect_text("created=1577836800 done", 5, &RenderZone::Utc);
    assert!(
        hits.iter()
            .any(|h| h.number == "1577836800"
                && h.readings.iter().any(|r| r.format_id.contains("unix"))),
        "{hits:?}"
    );
}

#[test]
fn inspect_text_short_hex_word_does_not_explode() {
    // A short hex-looking word (< 8 chars, below the byte floor) is NOT treated as
    // a hex token — it stays quiet rather than emitting spurious readings.
    let hits = scan::inspect_text("the cafe is nice", 5, &RenderZone::Utc);
    assert!(
        !hits.iter().any(|h| h.number == "cafe"),
        "short hex word must not decode: {hits:?}"
    );
}
