//! A selected zone's central meridian (standard UTC offset × 15°), used to
//! default the 干支 longitude. It is the *standard* meridian — DST is removed,
//! since the zone's geography doesn't move in summer — so New York is −75°
//! (75°W) in both seasons, and London 0° whether GMT or BST.
#![allow(clippy::unwrap_used)]

use timeglyph::{PosixNs, RenderZone};
use timeglyph_lens::tzinfo;

const SUMMER: PosixNs = PosixNs(1_625_097_600_000_000_000); // 2021-07-01T00:00Z
const WINTER: PosixNs = PosixNs(1_609_459_200_000_000_000); // 2021-01-01T00:00Z

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn offset_hours_reads_the_zone_offset() {
    let z = RenderZone::parse("+05:30").unwrap();
    assert!(approx(tzinfo::offset_hours(&z, WINTER).unwrap(), 5.5));
    assert!(approx(
        tzinfo::offset_hours(&RenderZone::Utc, WINTER).unwrap(),
        0.0
    ));
}

#[test]
fn meridian_of_offset_is_fifteen_degrees_per_hour() {
    // The shared offset→meridian formula (used by meridian_longitude and the map).
    assert!(approx(tzinfo::meridian_of_offset(8.0), 120.0));
    assert!(approx(tzinfo::meridian_of_offset(-5.0), -75.0));
}

#[test]
fn utc_meridian_is_zero() {
    assert!(approx(
        tzinfo::meridian_longitude(&RenderZone::Utc, SUMMER).unwrap(),
        0.0
    ));
}

#[test]
fn fixed_offset_meridian_is_offset_times_fifteen() {
    let z = RenderZone::parse("+05:30").unwrap();
    assert!(approx(
        tzinfo::meridian_longitude(&z, SUMMER).unwrap(),
        82.5
    ));
}

#[test]
fn named_zone_meridian_is_standard_and_dst_invariant() {
    let ny = RenderZone::parse("America/New_York").unwrap();
    // EST = UTC-5 → -75°. EDT (summer) removes DST → still -75°.
    assert!(approx(
        tzinfo::meridian_longitude(&ny, WINTER).unwrap(),
        -75.0
    ));
    assert!(approx(
        tzinfo::meridian_longitude(&ny, SUMMER).unwrap(),
        -75.0
    ));

    let london = RenderZone::parse("Europe/London").unwrap();
    // GMT = 0°; BST removes DST → still 0°.
    assert!(approx(
        tzinfo::meridian_longitude(&london, WINTER).unwrap(),
        0.0
    ));
    assert!(approx(
        tzinfo::meridian_longitude(&london, SUMMER).unwrap(),
        0.0
    ));
}
