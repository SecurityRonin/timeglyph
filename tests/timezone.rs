//! Selectable output timezone (presentation only).
//!
//! The instant ([`timeglyph::PosixNs`]) is absolute; a [`timeglyph::RenderZone`]
//! changes only how it is *displayed*. The default is UTC with a `Z` suffix
//! (unambiguous — "the offset to local time is unknown"). A fixed offset or a
//! named IANA zone renders with a numeric offset, DST-correct per instant.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::{format, PosixNs, RenderZone};

/// The instant 2020-01-01T00:00:00Z (a winter instant in the US).
fn winter() -> PosixNs {
    format("unix").unwrap().decode_int(1_577_836_800).unwrap()
}

/// The instant 2020-07-01T00:00:00Z (a summer instant in the US).
fn summer() -> PosixNs {
    format("unix").unwrap().decode_int(1_593_561_600).unwrap()
}

#[test]
fn default_zone_renders_utc_with_z() {
    let z = RenderZone::parse("UTC").unwrap();
    assert_eq!(winter().render(&z).as_deref(), Some("2020-01-01T00:00:00Z"));
    // Empty / "Z" are also UTC.
    assert!(matches!(RenderZone::parse("").unwrap(), RenderZone::Utc));
    assert!(matches!(RenderZone::parse("Z").unwrap(), RenderZone::Utc));
}

#[test]
fn fixed_offset_renders_numeric_offset() {
    let z = RenderZone::parse("+08:00").unwrap();
    assert_eq!(
        winter().render(&z).as_deref(),
        Some("2020-01-01T08:00:00+08:00")
    );
    // Compact and negative forms parse too.
    let z2 = RenderZone::parse("-0500").unwrap();
    assert_eq!(
        winter().render(&z2).as_deref(),
        Some("2019-12-31T19:00:00-05:00")
    );
}

#[test]
fn named_zone_is_dst_correct_per_instant() {
    let ny = RenderZone::parse("America/New_York").unwrap();
    // Winter → EST (-05:00); summer → EDT (-04:00). Same zone, different offset.
    assert_eq!(
        winter().render(&ny).as_deref(),
        Some("2019-12-31T19:00:00-05:00")
    );
    assert_eq!(
        summer().render(&ny).as_deref(),
        Some("2020-06-30T20:00:00-04:00")
    );
}

#[test]
fn unknown_zone_is_an_error_not_a_silent_utc_fallback() {
    // Secure-by-default: a typo'd zone must fail loudly, never silently render
    // UTC and mislead the analyst about the displayed offset.
    assert!(RenderZone::parse("Not/AZone").is_err());
    assert!(RenderZone::parse("PST8PDT_nope").is_err());
}
