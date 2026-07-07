//! Tests for the `copy_text_for` helper — the pure logic that decides which
//! string a click on a reading row copies to the clipboard.
//!
//! Correctness tier: T2 — constructed readings whose `rendered` / `instant`
//! values are derived from the documented construction; no independent oracle
//! beyond the round-trip through `timeglyph::datefmt`.
#![allow(clippy::unwrap_used)]

use timeglyph::scan::Reading;
use timeglyph::{DateStyle, PosixNs, RenderZone};
use timeglyph_lens::text::copy_text_for;

// A minimal Reading for tests — only the fields `copy_text_for` uses.
fn reading(rendered: &str, instant_ns: i128, local: bool) -> Reading {
    Reading {
        format_id: "unix".to_string(),
        rendered: rendered.to_string(),
        label: "Unix time (seconds)".to_string(),
        local,
        instant: PosixNs(instant_ns),
        score: 1.0,
        components: vec![],
    }
}

/// For a local-naive reading `copy_text_for` returns `r.rendered` unchanged —
/// the display zone is not applied (there is no UTC anchor to shift).
#[test]
fn local_reading_returns_rendered() {
    let r = reading("2021-07-01T12:34:56", 0, true);
    let got = copy_text_for(&r, &RenderZone::Utc, DateStyle::default());
    assert_eq!(got, "2021-07-01T12:34:56");
}

/// For a zone-shiftable reading in UTC, `copy_text_for` uses
/// `timeglyph::datefmt::format_instant` — not `r.rendered` — so the displayed
/// value reflects the active zone and style.
#[test]
fn utc_reading_uses_format_instant() {
    // 2021-01-01 00:00:00 UTC = 1_609_459_200 seconds
    let ns: i128 = 1_609_459_200 * 1_000_000_000;
    let r = reading("2021-01-01T00:00:00Z", ns, false);
    let got = copy_text_for(&r, &RenderZone::Utc, DateStyle::default());
    // The exact rendered form is whatever `format_instant` produces for UTC —
    // what matters is that it is NOT a copy of the stale `r.rendered` but a
    // fresh format of the instant in the requested zone/style.
    let expected =
        timeglyph::datefmt::format_instant(PosixNs(ns), &RenderZone::Utc, DateStyle::default());
    assert_eq!(got, expected);
}
