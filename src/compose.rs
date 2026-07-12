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

// --- Format wave 2 (engine-local decoders for additional encodings) ----------

/// Build an instant from naive UTC civil fields. Invalid fields (month 0, hour
/// 24, …) surface as an error, never a panic. Shared by the packed wave-2
/// decoders below.
fn civil_utc(
    year: i16,
    month: i8,
    day: i8,
    hour: i8,
    minute: i8,
    second: i8,
) -> Result<PosixNs, ChronoError> {
    let dt = jiff::civil::DateTime::new(year, month, day, hour, minute, second, 0)
        .map_err(|e| ChronoError::Render(e.to_string()))?;
    let ts = dt
        .to_zoned(jiff::tz::TimeZone::UTC)
        // cov:unreachable: to_zoned(UTC) of an already-valid civil datetime cannot fail.
        .map_err(|e| ChronoError::Render(e.to_string()))?
        .timestamp();
    Ok(PosixNs(ts.as_nanosecond()))
}

/// Oracle `DATE` — the 7-byte internal format `[century+100, year-of-century+100,
/// month, day, hour+1, minute+1, second+1]` (excess-100 on the two year bytes,
/// excess-1 on the time bytes; no timezone). Everywhere in `.dbf`/redo-log and
/// `.ibd` carving. Reference: Oracle DATE internal representation.
///
/// # Errors
/// [`ChronoError`] if the reconstructed civil datetime is invalid (never panics).
pub fn oracle_date(b: [u8; 7]) -> Result<PosixNs, ChronoError> {
    let year = (i16::from(b[0]) - 100) * 100 + (i16::from(b[1]) - 100);
    let f = |v: u8, off: i16| (i16::from(v) - off) as i8;
    civil_utc(
        year,
        f(b[2], 0),
        f(b[3], 0),
        f(b[4], 1),
        f(b[5], 1),
        f(b[6], 1),
    )
}

/// ISO 9660 / ECMA-119 §9.1.5 recording date — 7 bytes `[years since 1900, month,
/// day, hour, minute, second, offset from GMT in 15-minute intervals (signed)]`.
/// The offset is subtracted so the returned instant is absolute UTC. Optical-media
/// forensics.
///
/// # Errors
/// [`ChronoError`] if the civil fields are invalid (never panics).
pub fn iso9660(b: [u8; 7]) -> Result<PosixNs, ChronoError> {
    let year = 1900 + i16::from(b[0]);
    let civil = civil_utc(
        year, b[1] as i8, b[2] as i8, b[3] as i8, b[4] as i8, b[5] as i8,
    )?;
    // Byte 6: signed count of 15-minute units east of GMT; the instant is the wall
    // time minus that offset.
    let offset_ns = i128::from(b[6] as i8) * 15 * 60 * 1_000_000_000;
    Ok(PosixNs(civil.0 - offset_ns))
}

/// ext4 extended timestamp — a 32-bit seconds-since-1970 field plus a 32-bit
/// `extra` field: the low 2 bits extend the epoch by `×2^32` seconds (deferring
/// Y2038), the high 30 bits are nanoseconds. Linux filesystem forensics.
/// Reference: the ext4 on-disk inode (`i_[cma]time_extra`).
#[must_use]
pub fn ext4_extra(secs: i64, extra: u32) -> PosixNs {
    let epoch_bits = i128::from(extra & 0x3);
    let nanos = i128::from(extra >> 2);
    PosixNs((i128::from(secs) + (epoch_bits << 32)) * 1_000_000_000 + nanos)
}

/// IEC 60870-5 CP56Time2a — a 7-byte SCADA timestamp: `[milliseconds (LE u16,
/// 0-59999), minute (low 6 bits) + IV/GEN flags, hour (low 5 bits) + SU flag, day
/// of month (low 5 bits) + day of week, month (low 4 bits), year (low 7 bits,
/// +2000)]`. The validity / summer-time / day-of-week flag bits are masked off.
/// IEC 60870-5-101/-104 (the Industroyer/CrashOverride protocol family).
///
/// # Errors
/// [`ChronoError`] if the civil fields are invalid (never panics).
pub fn cp56time2a(b: [u8; 7]) -> Result<PosixNs, ChronoError> {
    let ms = u16::from(b[0]) | (u16::from(b[1]) << 8);
    let sub_ns = i128::from(ms % 1000) * 1_000_000;
    let inst = civil_utc(
        2000 + i16::from(b[6] & 0x7F),
        (b[5] & 0x0F) as i8,
        (b[4] & 0x1F) as i8,
        (b[3] & 0x1F) as i8,
        (b[2] & 0x3F) as i8,
        (ms / 1000) as i8,
    )?;
    Ok(PosixNs(inst.0 + sub_ns))
}

/// ECMA-167 (UDF) timestamp — 12 bytes: `[TypeAndTimezone (LE u16: low 12 bits =
/// signed minutes east of UTC, 0x800 = no tz), year (LE i16), month, day, hour,
/// minute, second, centiseconds, hundreds-of-microseconds, microseconds]`. The
/// timezone is subtracted so the returned instant is absolute UTC. UDF-media
/// forensics.
///
/// # Errors
/// [`ChronoError`] if the civil fields are invalid (never panics).
pub fn udf(b: [u8; 12]) -> Result<PosixNs, ChronoError> {
    let year = (u16::from(b[2]) | (u16::from(b[3]) << 8)) as i16;
    let civil = civil_utc(
        year, b[4] as i8, b[5] as i8, b[6] as i8, b[7] as i8, b[8] as i8,
    )?;
    let sub_ns =
        i128::from(b[9]) * 10_000_000 + i128::from(b[10]) * 100_000 + i128::from(b[11]) * 1_000;
    let tz12 = (u16::from(b[0]) | (u16::from(b[1]) << 8)) & 0x0FFF;
    // Sign-extend the 12-bit timezone (minutes east of UTC); 0 and 0x800 = no tz.
    let tz_min = if tz12 == 0 || tz12 == 0x800 {
        0
    } else if tz12 & 0x800 != 0 {
        i32::from(tz12) - 0x1000
    } else {
        i32::from(tz12)
    };
    Ok(PosixNs(
        civil.0 + sub_ns - i128::from(tz_min) * 60 * 1_000_000_000,
    ))
}
