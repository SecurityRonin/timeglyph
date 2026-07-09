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
