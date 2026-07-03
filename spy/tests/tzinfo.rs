//! Tests for per-instant zone stamping (offset · abbreviation · DST).
#![allow(clippy::unwrap_used)]

use timeglyph::{PosixNs, RenderZone};
use timeglyph_spy::tzinfo;

// A named zone resolves offset/abbreviation/DST *per instant* — the whole point:
// a location alone is ambiguous across summer/winter.
const SUMMER: PosixNs = PosixNs(1_625_097_600_000_000_000); // 2021-07-01T00:00Z
const WINTER: PosixNs = PosixNs(1_609_459_200_000_000_000); // 2021-01-01T00:00Z

#[test]
fn named_zone_stamp_differs_across_dst() {
    let ny = RenderZone::parse("America/New_York").unwrap();

    let s = tzinfo::stamp(&ny, SUMMER).unwrap();
    assert_eq!(s.abbr, "EDT");
    assert!(s.dst);
    assert!(s.offset.contains("-04"), "{}", s.offset);

    let w = tzinfo::stamp(&ny, WINTER).unwrap();
    assert_eq!(w.abbr, "EST");
    assert!(!w.dst);
    assert!(w.offset.contains("-05"), "{}", w.offset);
}

#[test]
fn london_is_gmt_in_winter_bst_in_summer() {
    let london = RenderZone::parse("Europe/London").unwrap();
    assert_eq!(tzinfo::stamp(&london, WINTER).unwrap().abbr, "GMT");
    let summer = tzinfo::stamp(&london, SUMMER).unwrap();
    assert_eq!(summer.abbr, "BST");
    assert!(summer.dst);
}

#[test]
fn fixed_offset_has_no_abbr_or_dst_but_keeps_offset() {
    let z = RenderZone::parse("+05:30").unwrap();
    let s = tzinfo::stamp(&z, SUMMER).unwrap();
    assert!(s.offset.contains("+05:30"));
    assert!(s.abbr.is_empty());
    assert!(!s.dst);
}

#[test]
fn utc_needs_no_stamp() {
    // UTC readings already show `Z`; there is nothing to add.
    assert!(tzinfo::stamp(&RenderZone::Utc, SUMMER).is_none());
}

#[test]
fn numeric_pseudo_abbreviation_is_suppressed() {
    // Zones with no traditional letter code (Acre, many others) report a numeric
    // "abbreviation" like `-05`, which just repeats the offset — suppress it so
    // the summary isn't `UTC-05 -05`.
    let acre = RenderZone::parse("America/Rio_Branco").unwrap();
    let s = tzinfo::stamp(&acre, WINTER).unwrap();
    assert!(s.offset.contains("-05"), "offset {}", s.offset);
    assert!(s.abbr.is_empty(), "expected no abbr, got {:?}", s.abbr);
}
