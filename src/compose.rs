//! Composite (two-word) timestamp decode: values split across two integer
//! fields. Some artifacts store a timestamp as two halves rather than one
//! integer — a FILETIME as its `dwLowDateTime`/`dwHighDateTime` DWORDs in `.reg`
//! exports, IE `index.dat` cookies, and packed malware configs. This reassembles
//! the halves and decodes via the canonical single-value path, so the same
//! epoch math applies. No single-value converter reconstructs these.

use crate::{ChronoError, PosixNs, Unit};

/// Reconstruct a Windows FILETIME from its low and high 32-bit halves and decode
/// it as 100 ns since 1601. `FILETIME = (high << 32) | low` — the order the two
/// DWORDs carry in a `FILETIME`/`Windows Cookie` structure.
///
/// # Errors
/// Returns [`ChronoError`] if the reconstructed value is out of the decodable
/// range (never panics).
pub fn filetime_hilo(low: u32, high: u32) -> Result<PosixNs, ChronoError> {
    let ft = (u64::from(high) << 32) | u64::from(low);
    let ticks = i64::try_from(ft).map_err(|_| ChronoError::OutOfRange {
        what: "filetime",
        value: i128::from(ft),
    })?;
    crate::format("filetime")?.decode_int(ticks)
}

/// Reconstruct a leap-correct UTC reading from a GPS `(week, time-of-week)` pair —
/// the native form of GNSS receiver time (u-blox, NMEA, Berla iVe vehicle
/// extractions, drone flight logs). `gps_seconds = week × 604800 + tow`, then
/// GPS↔UTC via the leap-second table (GPS itself has no leap seconds). Returns a
/// [`crate::leap::LeapReading`], deliberately outside the [`PosixNs`] spine.
#[cfg(feature = "leap")]
#[must_use]
pub fn gps_week_tow(week: u32, tow: f64) -> crate::leap::LeapReading {
    crate::leap::from_gps_seconds(f64::from(week) * 604_800.0 + tow)
}

/// Reconstruct a VMware snapshot time from a `.vmsd` `createTimeHigh`/
/// `createTimeLow` pair: microseconds since 1970 split across two 32-bit fields,
/// the low half stored as a signed `i32`. `us = (high << 32) | (low as u32)`.
/// Total (fits [`PosixNs`]'s i128).
#[must_use]
pub fn vmsd(high: i32, low: i32) -> PosixNs {
    let us = (i128::from(high) << 32) | i128::from(low.cast_unsigned());
    PosixNs(us * 1_000)
}

/// An instant `ticks` × `unit` after an `anchor` — for boot/epoch-relative times
/// whose stored value is a *duration*, not an absolute instant: Android
/// `elapsedRealtime` (ms since boot), Apple mach continuous time (ns since boot),
/// kernel uptime jiffies. The anchor (e.g. the boot instant) must be supplied
/// separately because the value alone cannot place the event on a calendar.
///
/// Total: `anchor.0 (i128) + i64 × unit-ns (i128)` stays within [`PosixNs`]'s i128.
#[must_use]
pub fn relative(anchor: PosixNs, ticks: i64, unit: Unit) -> PosixNs {
    PosixNs(anchor.0 + i128::from(ticks) * unit.nanos())
}

/// Reconstruct a Unix timestamp from a `(seconds, nanoseconds)` pair — a
/// `struct timespec` as stored by ext4/BTRFS/ZFS/XFS `stat`, protobuf
/// `google.protobuf.Timestamp`, and Java `Instant`. `PosixNs = sec*1e9 + nsec`.
///
/// Total (never fails): `i64 * 1e9 + u32` always fits [`PosixNs`]'s `i128`.
#[must_use]
pub fn unix_sec_nsec(sec: i64, nsec: u32) -> PosixNs {
    PosixNs(i128::from(sec) * 1_000_000_000 + i128::from(nsec))
}
