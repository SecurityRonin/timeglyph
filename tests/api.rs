//! The stable one-call public API: `identify(value)` — the entry point library
//! consumers (bindings, WASM playground, integrations) build on. Merges integer,
//! float, and string families and ranks by score. This is the contract those
//! embeddings depend on, so it is tested in its own right.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret::identify;

#[test]
fn identify_ranks_unix_first_for_a_2020_value() {
    let cands = identify("1577836800");
    assert_eq!(cands[0].format_id, "unix");
    assert!(cands[0].rendered.as_deref().unwrap().contains("2020-01-01"));
}

#[test]
fn identify_merges_float_and_string_families() {
    // A Unix-float string surfaces unix_float (not just cocoa_float)...
    assert!(identify("1712345678.001200")
        .iter()
        .any(|c| c.format_id == "unix_float"));
    // ...and a self-describing string form is identified too.
    assert!(identify("D:20260709123456+08'00'")
        .iter()
        .any(|c| c.format_id == "pdf_date"));
}

#[test]
fn identify_returns_empty_for_undecodable_input() {
    assert!(identify("not a timestamp at all !!!").is_empty());
}

#[test]
fn identify_is_sorted_by_score_descending() {
    let cands = identify("1300000000");
    assert!(cands.windows(2).all(|w| w[0].score >= w[1].score));
}

#[test]
fn identify_json_is_a_parseable_array_of_readings() {
    // The JSON-in/JSON-out boundary the WASM playground and other bindings call.
    let json = timeglyph::interpret::identify_json("1577836800");
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let arr = v.as_array().expect("array");
    assert!(arr.iter().any(|r| r["format_id"] == "unix"));
    assert!(arr[0]["citation"].as_str().is_some());
    // Undecodable input is an empty array, never an error.
    assert_eq!(timeglyph::interpret::identify_json("nope!!!"), "[]");
}

fn iso(s: &str) -> timeglyph::PosixNs {
    timeglyph::interpret::interpret_string(s)
        .into_iter()
        .find(|c| c.format_id == "iso8601")
        .unwrap()
        .instant
}

#[test]
fn syslog_infers_year_from_a_reference() {
    use timeglyph::interpret::parse_syslog_with_reference;
    // RFC 3164 syslog omits the year. Jan 12 with a March-2026 reference is the
    // same year (before the reference).
    let r = parse_syslog_with_reference("Jan 12 06:30:00", iso("2026-03-01T00:00:00Z")).unwrap();
    assert_eq!(
        r.render(&timeglyph::RenderZone::Utc).unwrap(),
        "2026-01-12T06:30:00Z"
    );
    // Dec 25 with a January-2026 reference resolves to the PRIOR year (Dec 25
    // 2026 would be in the future relative to the reference).
    let r2 =
        parse_syslog_with_reference("Dec 25 06:30:00", iso("2026-01-15T00:00:00Z")).unwrap();
    assert_eq!(
        r2.render(&timeglyph::RenderZone::Utc).unwrap(),
        "2025-12-25T06:30:00Z"
    );
}
