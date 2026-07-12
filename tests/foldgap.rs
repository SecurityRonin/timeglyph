//! DST fold/gap resolution for LocalNaive wall-clock values (correctness wave).
//!
//! A `LocalNaive` format (FAT, EXIF, DOSDATE…) stores civil wall-clock with no
//! offset. Interpreting those fields *in* a concrete zone (via `--tz`) is where
//! DST ambiguity appears: a fall-back "fold" maps one wall time to TWO instants;
//! a spring-forward "gap" maps it to NONE. These are properties of the IANA tzdb
//! (tier-1: the transition instants are authored by the zone database), so the
//! expected instants below are hand-derivable from the documented US transitions.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::{resolve_local, LocalResolution, PosixNs, RenderZone};

/// Build a naive instant from whole Unix seconds read as civil-UTC.
fn naive(secs: i64) -> PosixNs {
    PosixNs(i128::from(secs) * 1_000_000_000)
}

#[test]
fn fall_back_wall_time_is_a_fold_with_two_instants() {
    // 2021-11-07, America/New_York: clocks fall back 02:00 EDT → 01:00 EST, so the
    // wall time 01:30 occurs twice. Naive civil 2021-11-07T01:30:00 = unix 1636248600.
    let ny = RenderZone::parse("America/New_York").unwrap();
    match resolve_local(naive(1_636_248_600), &ny) {
        LocalResolution::Fold { earlier, later } => {
            // earlier = 01:30 EDT (-04:00) = 05:30Z = unix 1636263000
            // later   = 01:30 EST (-05:00) = 06:30Z = unix 1636266600
            assert_eq!(earlier, naive(1_636_263_000), "earlier (EDT) instant");
            assert_eq!(later, naive(1_636_266_600), "later (EST) instant");
        }
        other => panic!("expected Fold, got {other:?}"),
    }
}

#[test]
fn spring_forward_wall_time_is_a_gap() {
    // 2022-03-13, America/New_York: clocks spring forward 02:00 EST → 03:00 EDT, so
    // 02:30 never exists. Naive civil 2022-03-13T02:30:00 = unix 1647138600.
    let ny = RenderZone::parse("America/New_York").unwrap();
    assert!(matches!(
        resolve_local(naive(1_647_138_600), &ny),
        LocalResolution::Gap
    ));
}

#[test]
fn ordinary_wall_time_is_unique() {
    // 2021-06-15T12:00:00 in NY is unambiguous (EDT, -04:00) → 16:00Z = unix 1623772800.
    let ny = RenderZone::parse("America/New_York").unwrap();
    assert_eq!(
        resolve_local(naive(1_623_758_400), &ny),
        LocalResolution::Unique(naive(1_623_772_800))
    );
}

#[test]
fn utc_zone_never_folds_or_gaps() {
    // With no real zone (UTC), a naive value is already the instant — always unique.
    assert_eq!(
        resolve_local(naive(1_636_248_600), &RenderZone::Utc),
        LocalResolution::Unique(naive(1_636_248_600))
    );
}

#[test]
fn fixed_offset_shifts_the_naive_civil_by_the_offset() {
    // Civil 08:00 at +08:00 is 00:00Z. Naive 2021-06-15T08:00:00 = unix 1623744000;
    // minus 8h = unix 1623715200 (2021-06-15T00:00:00Z).
    let plus8 = RenderZone::parse("+08:00").unwrap();
    assert_eq!(
        resolve_local(naive(1_623_744_000), &plus8),
        LocalResolution::Unique(naive(1_623_715_200))
    );
}
