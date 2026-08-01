//! The behavioural contract of `timeglyph-core`'s epoch converters.
//!
//! Provenance discipline: no expected value below was arrived at by hand. Each
//! is either (a) an epoch offset published in a primary specification, or (b) an
//! instant that an implementation independent of this crate already decodes the
//! same way — `timeglyph`'s own jiff + forensicnomicon decode path, asserted in
//! this repository at `src/lib.rs` (crate doctest) and `src/secs.rs` (unit
//! tests). Each test names which.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph_core::{
    cocoa_secs_to_unix_ns, filetime_to_unix_ns, hfs_secs_to_unix_ns, webkit_micros_to_unix_ns,
    COCOA_EPOCH_OFFSET_SECS, FILETIME_EPOCH_OFFSET, HFS_EPOCH_OFFSET_SECS, WEBKIT_EPOCH_OFFSET,
};

// ---------------------------------------------------------------------------
// Windows FILETIME — 100-ns ticks since 1601-01-01 UTC ([MS-DTYP] §2.3.3).
// ---------------------------------------------------------------------------

/// The 1601→1970 span is 134,774 days = 11,644,473,600 s = 116,444,736,000,000,000
/// ticks. Independently asserted in this repo by `secs::tests::
/// filetime_epoch_offset_is_unix_zero`, which reaches the same number through
/// jiff and the forensicnomicon format table.
#[test]
fn filetime_epoch_offset_is_the_unix_epoch() {
    assert_eq!(FILETIME_EPOCH_OFFSET, 116_444_736_000_000_000);
    assert_eq!(filetime_to_unix_ns(FILETIME_EPOCH_OFFSET), Some(0));
}

/// `132_223_104_000_000_000` is 2020-01-01T00:00:00Z — the value timeglyph's
/// own crate-level doctest (`src/lib.rs`) decodes and renders as that instant,
/// through an implementation that shares no code with this crate.
#[test]
fn filetime_known_value_is_2020_01_01() {
    assert_eq!(
        filetime_to_unix_ns(132_223_104_000_000_000),
        Some(1_577_836_800_000_000_000)
    );
}

/// A zeroed FILETIME field is the "not set" sentinel, not 1601-01-01. This is
/// the contract that differs from `timeglyph::secs::filetime`, whose scanner
/// semantics deliberately report the 1601 instant.
#[test]
fn filetime_zero_is_the_not_set_sentinel() {
    assert_eq!(filetime_to_unix_ns(0), None);
}

/// One tick short of the Unix epoch: a real 1601-based instant, but below the
/// offset, so the sentinel policy rejects it rather than return a negative.
#[test]
fn filetime_below_the_unix_epoch_is_none() {
    assert_eq!(filetime_to_unix_ns(FILETIME_EPOCH_OFFSET - 1), None);
    assert_eq!(filetime_to_unix_ns(1), None);
}

/// The last tick whose nanosecond value still fits `i64` (≈ 2262-04-11), and the
/// first that does not. Boundary derived from `i64::MAX / 100` plus the offset.
#[test]
fn filetime_nanosecond_overflow_is_none() {
    assert_eq!(
        filetime_to_unix_ns(208_678_456_368_547_758),
        Some(9_223_372_036_854_775_800)
    );
    assert_eq!(filetime_to_unix_ns(208_678_456_368_547_759), None);
    assert_eq!(filetime_to_unix_ns(u64::MAX), None);
}

// ---------------------------------------------------------------------------
// WebKit / Chrome — microseconds since 1601-01-01 UTC. Chromium spells the
// offset `kTimeTToMicrosecondsOffset` in `base/time/time.h`.
// ---------------------------------------------------------------------------

#[test]
fn webkit_epoch_offset_is_the_unix_epoch() {
    assert_eq!(WEBKIT_EPOCH_OFFSET, 11_644_473_600_000_000);
    assert_eq!(webkit_micros_to_unix_ns(WEBKIT_EPOCH_OFFSET), Some(0));
}

/// 2021-03-01T12:30:15Z. Its Unix-seconds value, 1_614_601_815, is independently
/// asserted in this repo by `secs::tests::civil_known_dates`
/// (`civil(2021, 3, 1, 12, 30, 15) == Some(1_614_601_815)`), which computes it
/// through jiff.
#[test]
fn webkit_known_value_is_2021_03_01() {
    assert_eq!(
        webkit_micros_to_unix_ns(13_259_075_415_000_000),
        Some(1_614_601_815_000_000_000)
    );
}

#[test]
fn webkit_zero_is_the_not_set_sentinel() {
    assert_eq!(webkit_micros_to_unix_ns(0), None);
}

#[test]
fn webkit_below_the_unix_epoch_is_none() {
    assert_eq!(webkit_micros_to_unix_ns(WEBKIT_EPOCH_OFFSET - 1), None);
}

/// Boundary derived from `i64::MAX / 1000` plus the offset.
#[test]
fn webkit_nanosecond_overflow_is_none() {
    assert_eq!(
        webkit_micros_to_unix_ns(20_867_845_636_854_775),
        Some(9_223_372_036_854_775_000)
    );
    assert_eq!(webkit_micros_to_unix_ns(20_867_845_636_854_776), None);
    assert_eq!(webkit_micros_to_unix_ns(u64::MAX), None);
}

// ---------------------------------------------------------------------------
// Cocoa / Core Data — f64 seconds since 2001-01-01 UTC. Apple spells the offset
// `NSTimeIntervalSince1970` in Foundation's `NSDate.h`.
// ---------------------------------------------------------------------------

#[test]
fn cocoa_epoch_offset_is_the_2001_reference_date() {
    assert_eq!(COCOA_EPOCH_OFFSET_SECS, 978_307_200);
}

/// The same instant as `webkit_known_value_is_2021_03_01`, half a second later:
/// 636_294_615.5 + 978_307_200 = 1_614_601_815.5 s. The whole-second part is the
/// jiff-derived value asserted by `secs::tests::civil_known_dates`; the half
/// second is what sub-second preservation means here.
#[test]
fn cocoa_known_value_preserves_the_half_second() {
    assert_eq!(
        cocoa_secs_to_unix_ns(636_294_615.5),
        Some(1_614_601_815_500_000_000)
    );
}

#[test]
fn cocoa_sub_second_survives_near_its_own_epoch() {
    assert_eq!(cocoa_secs_to_unix_ns(0.5), Some(978_307_200_500_000_000));
}

#[test]
fn cocoa_zero_is_the_not_set_sentinel() {
    assert_eq!(cocoa_secs_to_unix_ns(0.0), None);
    assert_eq!(cocoa_secs_to_unix_ns(-0.0), None);
}

/// Unlike the 1601-based formats, a negative Cocoa value is an ordinary date
/// (anything before 2001), so it decodes rather than tripping the sentinel.
#[test]
fn cocoa_negative_is_a_real_pre_2001_instant() {
    assert_eq!(cocoa_secs_to_unix_ns(-978_307_201.0), Some(-1_000_000_000));
    assert_eq!(cocoa_secs_to_unix_ns(-978_307_200.5), Some(-500_000_000));
}

#[test]
fn cocoa_non_finite_is_none() {
    assert_eq!(cocoa_secs_to_unix_ns(f64::NAN), None);
    assert_eq!(cocoa_secs_to_unix_ns(f64::INFINITY), None);
    assert_eq!(cocoa_secs_to_unix_ns(f64::NEG_INFINITY), None);
}

/// `as i64` saturates rather than wrapping, which would turn an absurd float
/// into a plausible-looking instant. The range is rejected explicitly instead.
#[test]
fn cocoa_out_of_i64_range_is_none() {
    assert_eq!(cocoa_secs_to_unix_ns(1e30), None);
    assert_eq!(cocoa_secs_to_unix_ns(-1e30), None);
}

// ---------------------------------------------------------------------------
// HFS+ — u32 seconds since 1904-01-01 (Apple TN1150, HFS Plus Volume Format).
// ---------------------------------------------------------------------------

/// The 1904→1970 span is 24,107 days = 2,082,844,800 s.
#[test]
fn hfs_epoch_offset_is_the_unix_epoch() {
    assert_eq!(HFS_EPOCH_OFFSET_SECS, 2_082_844_800);
    assert_eq!(hfs_secs_to_unix_ns(HFS_EPOCH_OFFSET_SECS), Some(0));
}

/// 3_660_681_600 s after 1904-01-01 is 2020-01-01T00:00:00Z — the same instant
/// as `filetime_known_value_is_2020_01_01`, whose Unix value comes from
/// timeglyph's own decode path.
#[test]
fn hfs_known_value_is_2020_01_01() {
    assert_eq!(
        hfs_secs_to_unix_ns(3_660_681_600),
        Some(1_577_836_800_000_000_000)
    );
}

#[test]
fn hfs_zero_is_the_not_set_sentinel() {
    assert_eq!(hfs_secs_to_unix_ns(0), None);
}

#[test]
fn hfs_below_the_unix_epoch_is_none() {
    assert_eq!(hfs_secs_to_unix_ns(HFS_EPOCH_OFFSET_SECS - 1), None);
}

/// A u32 second count cannot overflow `i64` nanoseconds (the largest possible
/// value is ≈ 2.21e18 ns), so the conversion is total above the epoch — there is
/// no overflow arm to reach, by construction rather than by luck.
#[test]
fn hfs_maximum_u32_still_converts() {
    assert_eq!(
        hfs_secs_to_unix_ns(u32::MAX),
        Some(2_212_122_495_000_000_000)
    );
}
