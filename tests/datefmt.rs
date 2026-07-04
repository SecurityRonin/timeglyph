//! Engine `DateStyle` / `format_instant` display-style tests.
//!
//! Tier-2 validation: the expected strings are jiff's documented strftime
//! output for a known instant, cross-checked against the crate's own
//! `PosixNs::render` for the `Iso8601` invariant (the two must agree byte for
//! byte, offset/`Z` included).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::datefmt::{format_instant, DateStyle};
use timeglyph::{PosixNs, RenderZone};

/// 2020-01-01T00:00:00Z in POSIX nanoseconds.
const NY2020: PosixNs = PosixNs(1_577_836_800_000_000_000);

#[test]
fn default_is_iso8601() {
    assert_eq!(DateStyle::default(), DateStyle::Iso8601);
}

#[test]
fn iso8601_utc_matches_examples() {
    assert_eq!(
        format_instant(NY2020, &RenderZone::Utc, DateStyle::Iso8601),
        "2020-01-01T00:00:00Z"
    );
}

#[test]
fn space_separated_utc_matches_examples() {
    assert_eq!(
        format_instant(NY2020, &RenderZone::Utc, DateStyle::SpaceSeparated),
        "2020-01-01 00:00:00 UTC"
    );
}

#[test]
fn rfc2822_utc_matches_examples() {
    assert_eq!(
        format_instant(NY2020, &RenderZone::Utc, DateStyle::Rfc2822),
        "Wed, 01 Jan 2020 00:00:00 +0000"
    );
}

#[test]
fn us_style_utc_matches_examples() {
    assert_eq!(
        format_instant(NY2020, &RenderZone::Utc, DateStyle::UsStyle),
        "01/01/2020 12:00:00 AM UTC"
    );
}

#[test]
fn iso8601_exactly_reproduces_render_utc() {
    let expected = NY2020.render(&RenderZone::Utc).unwrap();
    assert_eq!(
        format_instant(NY2020, &RenderZone::Utc, DateStyle::Iso8601),
        expected
    );
}

#[test]
fn iso8601_exactly_reproduces_render_fixed() {
    let zone = RenderZone::parse("+08:00").unwrap();
    let expected = NY2020.render(&zone).unwrap();
    assert_eq!(format_instant(NY2020, &zone, DateStyle::Iso8601), expected);
}

#[test]
fn iso8601_exactly_reproduces_render_named() {
    let zone = RenderZone::parse("America/New_York").unwrap();
    let expected = NY2020.render(&zone).unwrap();
    assert_eq!(format_instant(NY2020, &zone, DateStyle::Iso8601), expected);
}

#[test]
fn fixed_offset_shifts_displayed_time_all_styles() {
    let zone = RenderZone::parse("+08:00").unwrap();
    assert_eq!(
        format_instant(NY2020, &zone, DateStyle::SpaceSeparated),
        "2020-01-01 08:00:00 +08"
    );
    assert_eq!(
        format_instant(NY2020, &zone, DateStyle::Rfc2822),
        "Wed, 01 Jan 2020 08:00:00 +0800"
    );
    assert_eq!(
        format_instant(NY2020, &zone, DateStyle::UsStyle),
        "01/01/2020 08:00:00 AM +08"
    );
}

#[test]
fn named_zone_dst_correct_all_styles() {
    let zone = RenderZone::parse("America/New_York").unwrap();
    // 2020-01-01T00:00:00Z is 2019-12-31 19:00 EST.
    assert_eq!(
        format_instant(NY2020, &zone, DateStyle::SpaceSeparated),
        "2019-12-31 19:00:00 EST"
    );
    assert_eq!(
        format_instant(NY2020, &zone, DateStyle::Rfc2822),
        "Tue, 31 Dec 2019 19:00:00 -0500"
    );
    assert_eq!(
        format_instant(NY2020, &zone, DateStyle::UsStyle),
        "12/31/2019 07:00:00 PM EST"
    );
}

#[test]
fn pm_hour_renders_in_us_style() {
    // 2020-01-01T13:00:00Z -> 01:00:00 PM.
    let one_pm = PosixNs(1_577_836_800_000_000_000 + 13 * 3_600_000_000_000);
    assert_eq!(
        format_instant(one_pm, &RenderZone::Utc, DateStyle::UsStyle),
        "01/01/2020 01:00:00 PM UTC"
    );
}

#[test]
fn out_of_civil_range_is_surfaced_not_panicked() {
    // i128 far beyond jiff's civil range: every style must degrade to a marker,
    // never panic.
    let absurd = PosixNs(i128::MAX);
    for style in [
        DateStyle::Iso8601,
        DateStyle::SpaceSeparated,
        DateStyle::Rfc2822,
        DateStyle::UsStyle,
    ] {
        let out = format_instant(absurd, &RenderZone::Utc, style);
        assert!(!out.is_empty());
    }
}
