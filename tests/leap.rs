//! Leap-aware time-scale anchors (ADR 0003).
//!
//! GPS and TAI are genuinely leap-aware: their UTC rendering must apply the
//! IERS leap-second offsets (TAI−UTC = 37 s, GPS−UTC = 18 s since 2017-01-01),
//! delegated to `hifitime`. NTP is UTC-additive (no leap counting) with an
//! era-rollover caveat. Anchor inputs were derived independently (Python +
//! authoritative leap constants; the TAI64 base convention is confirmed against
//! D. J. Bernstein's libtai spec: label 2^62 == 1970-01-01 00:00:00 TAI).
//!
//! Run with: `cargo test --features leap`.
#![cfg(feature = "leap")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::leap;

#[test]
fn gps_epoch_and_leap_offset() {
    // GPS epoch (0 s) = 1980-01-06 UTC, where GPS−UTC was 0.
    assert!(leap::from_gps_seconds(0.0)
        .utc_rfc3339
        .starts_with("1980-01-06T00:00:00"));
    // 1_261_872_018 GPS s == 2020-01-01 UTC (continuous SI count + 18 leap s).
    let r = leap::from_gps_seconds(1_261_872_018.0);
    assert!(
        r.utc_rfc3339.starts_with("2020-01-01T00:00:00"),
        "GPS 1261872018 = {}, expected 2020-01-01T00:00:00",
        r.utc_rfc3339
    );
}

#[test]
fn tai64_label_to_utc() {
    // 2^62 + (unix_2020 + 37) = TAI 2020-01-01T00:00:37 = UTC 2020-01-01T00:00:00.
    let r = leap::from_tai64(4_611_686_020_005_224_741).unwrap();
    assert!(
        r.utc_rfc3339.starts_with("2020-01-01T00:00:00"),
        "TAI64 = {}, expected 2020-01-01T00:00:00",
        r.utc_rfc3339
    );
}

#[test]
fn ntp_seconds_to_utc() {
    // NTP epoch 1900-01-01; NTP−Unix = 2_208_988_800. Additive (UTC, no leaps).
    assert!(leap::from_ntp_seconds(2_208_988_800)
        .unwrap()
        .utc_rfc3339
        .starts_with("1970-01-01T00:00:00"));
    assert!(leap::from_ntp_seconds(3_786_825_600)
        .unwrap()
        .utc_rfc3339
        .starts_with("2020-01-01T00:00:00"));
}

#[test]
fn gps_reading_states_its_assumptions() {
    // Leap readings are evidence, not verdicts, and must NOT claim the POSIX
    // spine — each carries scale + citation + assumptions.
    let r = leap::from_gps_seconds(0.0);
    assert_eq!(r.scale, "GPS");
    assert!(!r.citation.is_empty());
    assert!(!r.assumptions.is_empty());
}

#[test]
fn leap_error_paths() {
    // Negative values are not valid TAI64/NTP encodings (unsigned) → loud error.
    assert!(leap::decode("tai64", -1).unwrap().is_err());
    assert!(leap::decode("ntp", -1).unwrap().is_err());
    // A TAI64 label whose seconds offset exceeds i64 → out of range.
    assert!(leap::from_tai64(u64::MAX).is_err());
    // NTP seconds beyond i64, and seconds that land outside the civil range.
    assert!(leap::from_ntp_seconds(u64::MAX).is_err());
    assert!(leap::from_ntp_seconds(400_000_000_000).is_err());
    // A non-leap id returns None from the dispatch (falls through to POSIX).
    assert!(leap::decode("filetime", 0).is_none());
}
