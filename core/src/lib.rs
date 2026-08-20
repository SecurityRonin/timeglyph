//! `timeglyph-core` — zero-dependency epoch arithmetic.
//!
//! The integer half of [`timeglyph`](https://docs.rs/timeglyph). Turning a
//! stored tick count into nanoseconds since the Unix epoch is a subtraction and
//! a multiplication; it needs no calendar. Rendering that instant as ISO-8601
//! does, so rendering stays upstairs. That seam is what lets this crate keep
//! zero dependencies and a 1.75 MSRV, and so be linked by a FOUNDATION crate or
//! a published low-MSRV parser at no cost to either (ADR-0019).
//!
//! # The sentinel policy
//!
//! These converters answer a *parser's* question — "what instant does this field
//! hold, if any?" — so they return [`None`], rather than a decoded value, for:
//!
//! - **`0`** — the "not set" sentinel every one of these formats uses in
//!   practice. An unset Windows FILETIME field is a zeroed field, not a record
//!   of 1601-01-01.
//! - **anything below the format's own epoch offset**, which cannot be a real
//!   instant in a 1601- or 1904-based encoding and is the signature of
//!   misaligned bytes rather than of an ancient event.
//! - **arithmetic that would overflow `i64` nanoseconds** (≈ year 2262).
//!
//! Cocoa is the documented exception to the second rule: its 2001 reference date
//! makes negative values ordinary — every date before 2001 is one — so only
//! `0.0` and the non-finite values are rejected.
//!
//! This is deliberately **not** the contract of `timeglyph`'s scanner, which
//! answers a different question — "what *could* this value decode to?" — and so
//! reports the 1601 instant for a zero FILETIME rather than hiding it. Neither
//! is a fallback for the other; pick by what your caller does with the answer.
//!
//! # Example
//!
//! ```
//! use timeglyph_core::filetime_to_unix_ns;
//!
//! // 2020-01-01T00:00:00Z, as a Windows FILETIME.
//! assert_eq!(
//!     filetime_to_unix_ns(132_223_104_000_000_000),
//!     Some(1_577_836_800_000_000_000)
//! );
//! // A zeroed field is "not set", not 1601-01-01.
//! assert_eq!(filetime_to_unix_ns(0), None);
//! ```

/// 100-ns ticks from the Windows FILETIME epoch (1601-01-01 UTC) to the Unix
/// epoch: 134,774 days = 11,644,473,600 seconds. See [MS-DTYP] §2.3.3.
///
/// [MS-DTYP]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-dtyp/2c57429b-fdd4-488f-b5fc-9e4cf020fcdf
pub const FILETIME_EPOCH_OFFSET: u64 = 116_444_736_000_000_000;

/// Microseconds from the WebKit/Chrome epoch to the Unix epoch. Chrome stores
/// its history, cookie and download times as microseconds since 1601-01-01 UTC —
/// the same epoch as FILETIME at a coarser resolution. Chromium spells this
/// constant `kTimeTToMicrosecondsOffset` in `base/time/time.h`.
pub const WEBKIT_EPOCH_OFFSET: u64 = 11_644_473_600_000_000;

/// Seconds from the Unix epoch *forward* to the Cocoa / Core Data reference date
/// (2001-01-01 UTC) — so it is added, not subtracted. Apple publishes it as
/// `NSTimeIntervalSince1970` in Foundation's `NSDate.h`.
pub const COCOA_EPOCH_OFFSET_SECS: i64 = 978_307_200;

/// Seconds from the HFS+ epoch (1904-01-01 UTC) to the Unix epoch: 24,107 days.
/// See Apple Technical Note TN1150, *HFS Plus Volume Format*.
pub const HFS_EPOCH_OFFSET_SECS: u32 = 2_082_844_800;

/// Unix nanoseconds from a 64-bit Windows `FILETIME` (100-ns ticks since
/// 1601-01-01 UTC).
///
/// [`None`] when `ft` is `0` (the "not set" sentinel), when it is below
/// [`FILETIME_EPOCH_OFFSET`], or when the nanosecond value overflows `i64`.
/// A zero field is subsumed by the offset test — it is the smallest value below
/// the epoch, not a case of its own.
#[must_use]
pub fn filetime_to_unix_ns(ft: u64) -> Option<i64> {
    let ticks = ft.checked_sub(FILETIME_EPOCH_OFFSET)?;
    i64::try_from(ticks).ok()?.checked_mul(100)
}

/// Unix nanoseconds from a WebKit/Chrome timestamp (microseconds since
/// 1601-01-01 UTC) — Chrome's `History`, `Cookies` and `Web Data` stores.
///
/// Same sentinel policy as [`filetime_to_unix_ns`], against
/// [`WEBKIT_EPOCH_OFFSET`].
#[must_use]
pub fn webkit_micros_to_unix_ns(us: u64) -> Option<i64> {
    let micros = us.checked_sub(WEBKIT_EPOCH_OFFSET)?;
    i64::try_from(micros).ok()?.checked_mul(1_000)
}

/// Unix nanoseconds from a Cocoa / Core Data timestamp (seconds since
/// 2001-01-01 UTC), preserving the sub-second part.
///
/// [`None`] for `0.0` (the "not set" sentinel, `-0.0` included), for NaN and the
/// infinities, and for a magnitude that will not fit `i64` nanoseconds. Negative
/// values are *not* rejected: in a 2001-based encoding they are ordinary dates
/// before 2001, unlike the 1601-based formats above.
///
/// The conversion is done in `f64`, which is the format's own representation, so
/// nothing is lost that the stored value had — near the present, an `f64`
/// seconds count already resolves to roughly 100 ns.
#[must_use]
pub fn cocoa_secs_to_unix_ns(secs: f64) -> Option<i64> {
    // `i64::MIN` and `i64::MAX + 1`, both exactly representable in f64. The `as`
    // cast SATURATES rather than wrapping, which would silently turn an absurd
    // float into a plausible-looking instant; bound it explicitly instead.
    const MIN_NS: f64 = -9_223_372_036_854_775_808.0;
    const MAX_NS: f64 = 9_223_372_036_854_775_808.0;

    if !secs.is_finite() || secs == 0.0 {
        return None;
    }
    let ns = (secs + COCOA_EPOCH_OFFSET_SECS as f64) * 1e9;
    let ns = ns.round();
    if !(MIN_NS..MAX_NS).contains(&ns) {
        return None;
    }
    Some(ns as i64)
}

/// Unix nanoseconds from an HFS+ date (seconds since 1904-01-01 UTC) — the
/// catalog record timestamps of an HFS+ volume.
///
/// [`None`] for `0` and for anything below [`HFS_EPOCH_OFFSET_SECS`]. There is no
/// overflow case: the largest `u32` second count above the epoch is ≈ 2.21e18 ns,
/// well inside `i64`, so the conversion is total by construction.
///
/// TN1150 stores these as GMT, with one documented exception — the volume
/// header's `createDate`, which is local time. Converting that field to a real
/// instant needs the volume's timezone and is the caller's problem, not this
/// function's.
#[must_use]
pub fn hfs_secs_to_unix_ns(secs: u32) -> Option<i64> {
    let unix = secs.checked_sub(HFS_EPOCH_OFFSET_SECS)?;
    Some(i64::from(unix) * 1_000_000_000)
}
