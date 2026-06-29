//! The forensic format registry.
//!
//! Covers the linear (LinearInt/LinearFloat), embedded-ID (`Strategy::Embedded`,
//! any unit), and packed (FAT/DOS) integer strategies; the SYSTEMTIME packed and
//! ULID/UUIDv1/RFC-2822/EXIF string forms live in `interpret.rs`. Each entry's
//! epoch and worked example are cross-validated against the MIT `time-decode`
//! oracle (tests/oracle.rs, tests/catalog.rs).
//!
//! Still TODO (obscure / packed-bitfield formats whose layouts `time-decode`'s
//! own `--formats` flags reverse but which need per-format unpackers): exFAT
//! (tz-offset byte), bitdate/dttm/logtime/ns40/moto/symantec/dvr, the BCD/GSM
//! semi-octet family, Sonyflake (10ms unit), and the leap-aware GPS/NTP/TAI
//! scales (already in `leap.rs`). See HANDOFF.md §5a.
//!
//! Every epoch_ns constant below is a CLEAN-ROOM fact from a primary spec, to be
//! cross-validated against the MIT `time-decode` oracle and each spec's worked
//! example (HANDOFF §"Validation"). NEVER sourced from decompiling DCode.

use crate::{
    ChronoError, Format,
    LeapSemantics::PosixIgnored,
    PosixNs, Strategy,
    TzSemantics::{LocalNaive, Utc},
    Unit,
};

/// Unpack a 32-bit FAT/DOS packed date+time into an instant. The high 16 bits
/// are the date word (`(year-1980) << 9 | month << 5 | day`), the low 16 are the
/// time word (`hour << 11 | minute << 5 | second/2`, i.e. 2-second resolution).
/// FAT stores LOCAL time with no offset; the value is read as a naive civil
/// datetime (the [`Format`] carries `LocalNaive` so callers are not misled).
/// Invalid packed fields (month 0, day 0, …) surface as an error, never a panic.
fn decode_fat_dos(value: i64) -> Result<PosixNs, ChronoError> {
    let packed = u32::try_from(value).map_err(|_| ChronoError::OutOfRange {
        what: "FAT/DOS packed value (not a u32)",
        value: i128::from(value),
    })?;
    let date = (packed >> 16) as u16;
    let time = (packed & 0xFFFF) as u16;
    let year = 1980 + ((date >> 9) & 0x7F) as i16;
    let month = ((date >> 5) & 0x0F) as i8;
    let day = (date & 0x1F) as i8;
    let hour = ((time >> 11) & 0x1F) as i8;
    let minute = ((time >> 5) & 0x3F) as i8;
    let second = ((time & 0x1F) * 2) as i8;
    let dt = jiff::civil::DateTime::new(year, month, day, hour, minute, second, 0)
        .map_err(|e| ChronoError::Render(e.to_string()))?;
    let ts = dt
        .to_zoned(jiff::tz::TimeZone::UTC)
        // cov:unreachable: to_zoned(UTC) of an already-valid civil datetime cannot fail.
        .map_err(|e| ChronoError::Render(e.to_string()))?
        .timestamp();
    Ok(PosixNs(ts.as_nanosecond()))
}

/// Build an instant from packed civil fields read as naive UTC (the [`Format`]
/// carries `LocalNaive` so callers know it is wall-clock, not real UTC). Invalid
/// fields surface as an error, never a panic. Shared by the packed decoders.
fn packed_civil(
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

/// exFAT 32-bit packed timestamp (MSB-first): year(7,+1980) month(4) day(5)
/// hour(5) minute(6) 2-second(5). LOCAL time. [Microsoft exFAT spec]
fn decode_exfat(value: i64) -> Result<PosixNs, ChronoError> {
    let p = u32::try_from(value).map_err(|_| ChronoError::OutOfRange {
        what: "exFAT packed value (not a u32)",
        value: i128::from(value),
    })?;
    packed_civil(
        1980 + ((p >> 25) & 0x7F) as i16,
        ((p >> 21) & 0x0F) as i8,
        ((p >> 16) & 0x1F) as i8,
        ((p >> 11) & 0x1F) as i8,
        ((p >> 5) & 0x3F) as i8,
        ((p & 0x1F) * 2) as i8,
    )
}

/// Microsoft DTTM 32-bit packed date (MSB-first): dayOfWeek(3, ignored)
/// year(9,+1900) month(4) day(5) hour(5) minute(6); no seconds. LOCAL time.
fn decode_dttm(value: i64) -> Result<PosixNs, ChronoError> {
    let p = u32::try_from(value).map_err(|_| ChronoError::OutOfRange {
        what: "DTTM packed value (not a u32)",
        value: i128::from(value),
    })?;
    packed_civil(
        1900 + ((p >> 20) & 0x1FF) as i16,
        ((p >> 16) & 0x0F) as i8,
        ((p >> 11) & 0x1F) as i8,
        ((p >> 6) & 0x1F) as i8,
        (p & 0x3F) as i8,
        0,
    )
}

/// Samsung/LG BitDate: the 4 bytes are byte-reversed, then MSB-first year(12)
/// month(4) day(5) hour(5) minute(6); no seconds. LOCAL time.
fn decode_bitdate(value: i64) -> Result<PosixNs, ChronoError> {
    let p = u32::try_from(value)
        .map_err(|_| ChronoError::OutOfRange {
            what: "BitDate packed value (not a u32)",
            value: i128::from(value),
        })?
        .swap_bytes();
    packed_civil(
        ((p >> 20) & 0xFFF) as i16,
        ((p >> 16) & 0x0F) as i8,
        ((p >> 11) & 0x1F) as i8,
        ((p >> 6) & 0x1F) as i8,
        (p & 0x3F) as i8,
        0,
    )
}

/// Bitwise Decimal: a decimal value bit-packed year(>>20) month(&15 at >>16)
/// day(&31 at >>11) hour(&31 at >>6) minute(&63); no seconds. LOCAL time.
fn decode_bitdec(value: i64) -> Result<PosixNs, ChronoError> {
    if value < 0 {
        return Err(ChronoError::OutOfRange {
            what: "Bitwise Decimal value (negative)",
            value: i128::from(value),
        });
    }
    packed_civil(
        i16::try_from(value >> 20).map_err(|_| ChronoError::OutOfRange {
            what: "Bitwise Decimal year",
            value: i128::from(value),
        })?,
        ((value >> 16) & 15) as i8,
        ((value >> 11) & 31) as i8,
        ((value >> 6) & 31) as i8,
        (value & 63) as i8,
        0,
    )
}

/// Binary-Coded-Decimal: 12 decimal digits as pairs YY(+2000) MM DD HH MM SS,
/// LOCAL time. The value is read as its zero-padded 12-digit decimal string.
fn decode_bcd(value: i64) -> Result<PosixNs, ChronoError> {
    if !(0..1_000_000_000_000).contains(&value) {
        return Err(ChronoError::OutOfRange {
            what: "BCD value (not 12 decimal digits)",
            value: i128::from(value),
        });
    }
    let s = format!("{value:012}");
    let pair = |i: usize| -> i8 { s[i..i + 2].parse().unwrap_or(-1) };
    packed_civil(
        2000 + i16::from(pair(0)),
        pair(2),
        pair(4),
        pair(6),
        pair(8),
        pair(10),
    )
}

/// Motorola 6-byte timestamp: one byte per field — year(+1970) month day hour
/// minute second. UTC.
fn decode_moto(value: i64) -> Result<PosixNs, ChronoError> {
    let v = u64::try_from(value)
        .ok()
        .filter(|v| *v <= 0xFFFF_FFFF_FFFF)
        .ok_or(ChronoError::OutOfRange {
            what: "Motorola (not a 6-byte value)",
            value: i128::from(value),
        })?;
    let byte = |sh: u32| ((v >> sh) & 0xFF) as i8;
    packed_civil(
        1970 + i16::from(((v >> 40) & 0xFF) as u8),
        byte(32),
        byte(24),
        byte(16),
        byte(8),
        byte(0),
    )
}

/// Symantec AV 6-byte timestamp: like Motorola, but the month byte is +1. UTC.
fn decode_symantec(value: i64) -> Result<PosixNs, ChronoError> {
    let v = u64::try_from(value)
        .ok()
        .filter(|v| *v <= 0xFFFF_FFFF_FFFF)
        .ok_or(ChronoError::OutOfRange {
            what: "Symantec (not a 6-byte value)",
            value: i128::from(value),
        })?;
    let byte = |sh: u32| ((v >> sh) & 0xFF) as i8;
    packed_civil(
        1970 + i16::from(((v >> 40) & 0xFF) as u8),
        byte(32).wrapping_add(1),
        byte(24),
        byte(16),
        byte(8),
        byte(0),
    )
}

/// DVR (WFS/DHFS) 32-bit packed timestamp (MSB-first): year(6,+2000) month(4)
/// day(5) hour(5) minute(6) second(6). LOCAL time.
fn decode_dvr(value: i64) -> Result<PosixNs, ChronoError> {
    let p = u32::try_from(value).map_err(|_| ChronoError::OutOfRange {
        what: "DVR packed value (not a u32)",
        value: i128::from(value),
    })?;
    packed_civil(
        2000 + ((p >> 26) & 0x3F) as i16,
        ((p >> 22) & 0x0F) as i8,
        ((p >> 17) & 0x1F) as i8,
        ((p >> 12) & 0x1F) as i8,
        ((p >> 6) & 0x3F) as i8,
        (p & 0x3F) as i8,
    )
}

/// Nokia S40 7-byte timestamp: year(BE u16) then month/day/hour/minute/second,
/// each a raw byte value. UTC.
fn decode_ns40(value: i64) -> Result<PosixNs, ChronoError> {
    let v = u64::try_from(value)
        .ok()
        .filter(|v| *v <= 0xFF_FFFF_FFFF_FFFF)
        .ok_or(ChronoError::OutOfRange {
            what: "Nokia S40 (not a 7-byte value)",
            value: i128::from(value),
        })?;
    let byte = |sh: u32| ((v >> sh) & 0xFF) as i8;
    let yr = i16::try_from((v >> 40) & 0xFFFF).map_err(|_| ChronoError::OutOfRange {
        what: "Nokia S40 year",
        value: i128::from(value),
    })?;
    packed_civil(yr, byte(32), byte(24), byte(16), byte(8), byte(0))
}

/// Nokia S40 LE: like ns40 but the year u16 is little-endian. UTC.
fn decode_ns40le(value: i64) -> Result<PosixNs, ChronoError> {
    let v = u64::try_from(value)
        .ok()
        .filter(|v| *v <= 0xFF_FFFF_FFFF_FFFF)
        .ok_or(ChronoError::OutOfRange {
            what: "Nokia S40 LE (not a 7-byte value)",
            value: i128::from(value),
        })?;
    let byte = |sh: u32| ((v >> sh) & 0xFF) as i8;
    let yr = i16::try_from((((v >> 40) & 0xFFFF) as u16).swap_bytes()).map_err(|_| {
        ChronoError::OutOfRange {
            what: "Nokia S40 LE year",
            value: i128::from(value),
        }
    })?;
    packed_civil(yr, byte(32), byte(24), byte(16), byte(8), byte(0))
}

/// JET LogTime 8-byte timestamp, reversed field order: sec min hour day month
/// year(+1900), then 2 filler bytes. UTC.
fn decode_logtime(value: i64) -> Result<PosixNs, ChronoError> {
    let v = u64::try_from(value).map_err(|_| ChronoError::OutOfRange {
        what: "JET LogTime (negative)",
        value: i128::from(value),
    })?;
    let byte = |sh: u32| ((v >> sh) & 0xFF) as i8;
    let yr = 1900 + ((v >> 16) & 0xFF) as i16;
    packed_civil(yr, byte(24), byte(32), byte(40), byte(48), byte(56))
}

/// Decode one nibble-swapped semi-octet byte/pair to its decimal value, or -1
/// (an invalid field that `packed_civil` will reject) when a nibble exceeds 9.
fn semi_pair(low: u8, high: u8) -> i8 {
    i8::try_from(low * 10 + high).unwrap_or(-1)
}

/// Semi-Octet decimal: 12 digits, each pair nibble-swapped → YY(+2000) MM DD HH
/// MM SS. LOCAL time.
fn decode_semioctet(value: i64) -> Result<PosixNs, ChronoError> {
    if !(0..1_000_000_000_000).contains(&value) {
        return Err(ChronoError::OutOfRange {
            what: "Semi-Octet (not 12 decimal digits)",
            value: i128::from(value),
        });
    }
    let s = format!("{value:012}");
    let b = s.as_bytes();
    let pair = |i: usize| semi_pair(b[i + 1] - b'0', b[i] - b'0');
    packed_civil(
        2000 + i16::from(pair(0)),
        pair(2),
        pair(4),
        pair(6),
        pair(8),
        pair(10),
    )
}

/// GSM 7-byte semi-octet timestamp: per byte nibble-swap → decimal, YY(+2000)
/// MM DD HH MM SS, then a timezone byte (ignored). UTC.
fn decode_gsm(value: i64) -> Result<PosixNs, ChronoError> {
    let v = u64::try_from(value)
        .ok()
        .filter(|v| *v <= 0xFF_FFFF_FFFF_FFFF)
        .ok_or(ChronoError::OutOfRange {
            what: "GSM (not a 7-byte value)",
            value: i128::from(value),
        })?;
    let semi = |sh: u32| {
        let byte = ((v >> sh) & 0xFF) as u8;
        semi_pair(byte & 0x0F, byte >> 4)
    };
    packed_civil(
        2000 + i16::from(semi(48)),
        semi(40),
        semi(32),
        semi(24),
        semi(16),
        semi(8),
    )
}

/// Nokia time LE: 4 bytes reversed, then a two's-complement count of seconds
/// remaining before 2050 (`to_int − 0xFFFF_FFFF + secs(1970→2050)`). UTC.
fn decode_nokiale(value: i64) -> Result<PosixNs, ChronoError> {
    let p = i64::from(
        u32::try_from(value)
            .map_err(|_| ChronoError::OutOfRange {
                what: "Nokia LE (not a u32)",
                value: i128::from(value),
            })?
            .swap_bytes(),
    );
    let unix = p - 0xFFFF_FFFF + 2_524_608_000; // secs(1970→2050)
    Ok(PosixNs(i128::from(unix) * 1_000_000_000))
}

// Epoch offsets, in nanoseconds relative to the Unix epoch (1970-01-01).
// (seconds between the format epoch and 1970-01-01) × 1e9.
const NS: i128 = 1_000_000_000;
const FILETIME_EPOCH_NS: i128 = -11_644_473_600 * NS; // 1601-01-01  [MS-DTYP]
const COCOA_EPOCH_NS: i128 = 978_307_200 * NS; //        2001-01-01  (CFAbsoluteTime)
const HFS_EPOCH_NS: i128 = -2_082_844_800 * NS; //       1904-01-01  (HFS+ TN1150)
const DOTNET_EPOCH_NS: i128 = -62_135_596_800 * NS; //   0001-01-01  (.NET DateTime.Ticks)
const OLE_EPOCH_NS: i128 = -2_209_161_600 * NS; //       1899-12-30  (OLE Automation)
const POSTGRES_EPOCH_NS: i128 = 946_684_800 * NS; //     2000-01-01  (PostgreSQL timestamp)
                                                  // Julian Day 0 = noon, 24 Nov 4714 BC (proleptic Gregorian). unix_seconds(JD 0)
                                                  // = (0 - 2440587.5) × 86400, since JD 2440587.5 == the Unix epoch (SQLite docs).
const JULIAN_EPOCH_NS: i128 = -210_866_760_000 * NS;
// Snowflake-ID epochs, stored in ns (the scheme epoch is published in ms).
const MS: i128 = 1_000_000;
const TWITTER_EPOCH_NS: i128 = 1_288_834_974_657 * MS; // 2010-11-04 (Twitter/X)
const DISCORD_EPOCH_NS: i128 = 1_420_070_400_000 * MS; // 2015-01-01 (Discord)
const SONY_EPOCH_NS: i128 = 1_409_529_600 * NS; //        2014-09-01 (Sonyflake, 10ms units)
                                                // KSUID epoch: Unix second 1_400_000_000 == 2014-05-13T16:53:20Z (Segment KSUID).
const KSUID_EPOCH_NS: i128 = 1_400_000_000 * NS;

// Plausibility window for auto-detect ranking: 1990-01-01 .. 2040-01-01.
// NOT a filter on truth — only a prior on which readings to surface first.
const W_FROM: i128 = 631_152_000 * NS; // 1990-01-01
const W_TO: i128 = 2_208_988_800 * NS; // 2040-01-01
const W: (i128, i128) = (W_FROM, W_TO);

/// All registered formats (scaffold subset).
pub static FORMATS: &[Format] = &[
    Format {
        id: "unix",
        label: "Unix time (seconds)",
        family: "POSIX / Linux / web",
        strategy: Strategy::LinearInt {
            epoch_ns: 0,
            unit: Unit::Seconds,
        },
        citation: "POSIX.1-2017 §4.16",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "unix_ms",
        label: "Unix time (milliseconds, Java/JS)",
        family: "Java, JavaScript Date",
        strategy: Strategy::LinearInt {
            epoch_ns: 0,
            unit: Unit::Millis,
        },
        citation: "ECMA-262 (Date)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "unix_us",
        label: "Unix time (microseconds)",
        family: "various (sqlite, syslog)",
        strategy: Strategy::LinearInt {
            epoch_ns: 0,
            unit: Unit::Micros,
        },
        citation: "derived (Unix epoch, µs)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "filetime",
        label: "Windows FILETIME (100ns since 1601)",
        family: "NTFS, Registry, Event Log, AD",
        strategy: Strategy::LinearInt {
            epoch_ns: FILETIME_EPOCH_NS,
            unit: Unit::HundredNanos,
        },
        citation: "[MS-DTYP] §2.3.3 FILETIME",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "webkit",
        label: "Chrome / WebKit (µs since 1601)",
        family: "Chromium history/cookies",
        strategy: Strategy::LinearInt {
            epoch_ns: FILETIME_EPOCH_NS,
            unit: Unit::Micros,
        },
        citation: "Chromium base::Time (Windows epoch, µs)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "cocoa",
        label: "Cocoa / CFAbsoluteTime (s since 2001)",
        family: "macOS/iOS, NSDate, Core Data",
        strategy: Strategy::LinearInt {
            epoch_ns: COCOA_EPOCH_NS,
            unit: Unit::Seconds,
        },
        citation: "Apple Foundation NSDate (CFAbsoluteTime)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "hfsplus",
        label: "Apple HFS+ (s since 1904)",
        family: "HFS+ filesystem",
        strategy: Strategy::LinearInt {
            epoch_ns: HFS_EPOCH_NS,
            unit: Unit::Seconds,
        },
        citation: "Apple TN1150 (HFS Plus)",
        tz: Utc, // NB: classic-Mac HFS stored LOCAL; HFS+ is UTC. See HANDOFF.
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "dotnet_ticks",
        label: ".NET DateTime.Ticks (100ns since 0001)",
        family: ".NET / SQL Server datetime2",
        strategy: Strategy::LinearInt {
            epoch_ns: DOTNET_EPOCH_NS,
            unit: Unit::HundredNanos,
        },
        citation: "ECMA-335 / .NET DateTime.Ticks",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "ole",
        label: "OLE Automation date (days since 1899-12-30)",
        family: "Excel, COM, VARIANT DATE",
        strategy: Strategy::LinearFloat {
            epoch_ns: OLE_EPOCH_NS,
            unit: Unit::Days,
        },
        citation: "MS OLE Automation (DATE / VT_DATE)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "unix_ns",
        label: "Unix time (nanoseconds)",
        family: "Go time.UnixNano, APFS on-disk",
        strategy: Strategy::LinearInt {
            epoch_ns: 0,
            unit: Unit::Nanos,
        },
        citation: "derived (Unix epoch, ns); Apple APFS reference",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "postgres",
        label: "PostgreSQL timestamp (µs since 2000)",
        family: "PostgreSQL (64-bit integer datetimes)",
        strategy: Strategy::LinearInt {
            epoch_ns: POSTGRES_EPOCH_NS,
            unit: Unit::Micros,
        },
        citation: "PostgreSQL src timestamp.h (POSTGRES_EPOCH_JDATE)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "cocoa_float",
        label: "Cocoa CFAbsoluteTime (signed double, s since 2001)",
        family: "macOS/iOS plists, NSKeyedArchiver, Core Data",
        strategy: Strategy::LinearFloat {
            epoch_ns: COCOA_EPOCH_NS,
            unit: Unit::Seconds,
        },
        citation: "Apple CoreFoundation CFAbsoluteTime (CFDateGetAbsoluteTime)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "sqlite_julian",
        label: "SQLite Julian day (float days)",
        family: "SQLite julianday() / REAL date storage",
        strategy: Strategy::LinearFloat {
            epoch_ns: JULIAN_EPOCH_NS,
            unit: Unit::Days,
        },
        citation: "SQLite date-and-time functions (Julian day number)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "snowflake",
        label: "Twitter/X Snowflake ID (ms since 2010, <<22)",
        family: "Twitter/X object IDs",
        strategy: Strategy::Embedded {
            epoch_ns: TWITTER_EPOCH_NS,
            shift_bits: 22,
            unit: Unit::Millis,
        },
        citation: "Twitter Snowflake (epoch 1288834974657 ms, 22-bit shift)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "discord",
        label: "Discord Snowflake ID (ms since 2015, <<22)",
        family: "Discord object IDs",
        strategy: Strategy::Embedded {
            epoch_ns: DISCORD_EPOCH_NS,
            shift_bits: 22,
            unit: Unit::Millis,
        },
        citation: "Discord developer docs (epoch 1420070400000 ms, 22-bit shift)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "fat",
        label: "FAT/DOS packed date+time (LOCAL time)",
        family: "FAT/exFAT, ZIP, DOS",
        strategy: Strategy::Packed(decode_fat_dos),
        citation: "Microsoft FAT spec / ECMA-107 (DOS date/time fields)",
        // FAT stores wall-clock LOCAL time with NO offset — the rendered instant
        // is naive and must not be assumed UTC.
        tz: LocalNaive,
        leap: PosixIgnored,
        plausible: W,
    },
    // --- Catalog build-out (HANDOFF §5a), each cross-checked vs the MIT
    // `time-decode` oracle (tests/oracle.rs, tests/catalog.rs). --------------
    Format {
        id: "active",
        label: "Active Directory / LDAP (100ns since 1601)",
        family: "Active Directory, LDAP (lastLogon, pwdLastSet)",
        strategy: Strategy::LinearInt {
            epoch_ns: FILETIME_EPOCH_NS,
            unit: Unit::HundredNanos,
        },
        citation: "[MS-DTYP] §2.3.3 FILETIME (AD Integer8 date attributes)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "prtime",
        label: "Mozilla PRTime (µs since 1970)",
        family: "Firefox places.sqlite, Mozilla NSPR",
        strategy: Strategy::LinearInt {
            epoch_ns: 0,
            unit: Unit::Micros,
        },
        citation: "Mozilla NSPR PRTime (microseconds since the Unix epoch)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "iostime",
        label: "Apple NSDate iOS 11+ (ns since 2001)",
        family: "iOS 11+ Cocoa nanosecond NSDate",
        strategy: Strategy::LinearInt {
            epoch_ns: COCOA_EPOCH_NS,
            unit: Unit::Nanos,
        },
        citation: "Apple Foundation NSDate (CFAbsoluteTime), nanosecond form",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "ksuid",
        label: "KSUID timestamp (s since 2014-05-13)",
        family: "Segment KSUID (k-sortable unique IDs)",
        strategy: Strategy::LinearInt {
            epoch_ns: KSUID_EPOCH_NS,
            unit: Unit::Seconds,
        },
        citation: "Segment KSUID (epoch 1_400_000_000 = 2014-05-13T16:53:20Z)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "excel1904",
        label: "Microsoft Excel 1904 date (float days since 1904-01-01)",
        family: "Excel (legacy Mac 1904 date system)",
        strategy: Strategy::LinearFloat {
            epoch_ns: HFS_EPOCH_NS,
            unit: Unit::Days,
        },
        citation: "Microsoft Excel 1904 date system (serial day 0 = 1904-01-01)",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "mastodon",
        label: "Mastodon Snowflake ID (ms since 1970, <<16)",
        family: "Mastodon status / object IDs",
        strategy: Strategy::Embedded {
            epoch_ns: 0,
            shift_bits: 16,
            unit: Unit::Millis,
        },
        citation: "Mastodon Snowflake (Unix-ms epoch, 16-bit shift); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "linkedin",
        label: "LinkedIn activity ID (ms since 1970, <<22)",
        family: "LinkedIn activity / URN IDs",
        strategy: Strategy::Embedded {
            epoch_ns: 0,
            shift_bits: 22,
            unit: Unit::Millis,
        },
        citation: "LinkedIn activity timestamp (Unix-ms epoch, 22-bit shift); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "tiktok",
        label: "TikTok Snowflake ID (s since 1970, <<32)",
        family: "TikTok object IDs",
        strategy: Strategy::Embedded {
            epoch_ns: 0,
            shift_bits: 32,
            unit: Unit::Seconds,
        },
        citation: "TikTok ID (Unix-seconds epoch, 32-bit shift); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    // --- Packed bit-field formats (HANDOFF §5a long tail), LOCAL/naive time,
    // each cross-checked vs the MIT time-decode oracle (tests/packed.rs). ------
    Format {
        id: "exfat",
        label: "exFAT packed timestamp (LOCAL time)",
        family: "exFAT filesystem",
        strategy: Strategy::Packed(decode_exfat),
        citation: "Microsoft exFAT spec (32-bit packed timestamp); vs time-decode",
        tz: LocalNaive,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "dttm",
        label: "Microsoft DTTM packed date (LOCAL time)",
        family: "Microsoft Compound File / Office DTTM",
        strategy: Strategy::Packed(decode_dttm),
        citation: "Microsoft DTTM packed date (year since 1900); vs time-decode",
        tz: LocalNaive,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "bitdate",
        label: "Samsung/LG BitDate (byte-reversed packed, LOCAL time)",
        family: "Samsung / LG device timestamps",
        strategy: Strategy::Packed(decode_bitdate),
        citation: "Samsung/LG BitDate (byte-reversed 32-bit packed); vs time-decode",
        tz: LocalNaive,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "bitdec",
        label: "Bitwise Decimal packed date (LOCAL time)",
        family: "Bitwise Decimal packed timestamps",
        strategy: Strategy::Packed(decode_bitdec),
        citation: "Bitwise Decimal (decimal bit-packed date); vs time-decode",
        tz: LocalNaive,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "bcd",
        label: "Binary-Coded-Decimal YYMMDDHHMMSS (LOCAL time)",
        family: "BCD digit-pair timestamps",
        strategy: Strategy::Packed(decode_bcd),
        citation: "Binary-Coded-Decimal (YY+2000 MM DD HH MM SS pairs); vs time-decode",
        tz: LocalNaive,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "moto",
        label: "Motorola 6-byte timestamp",
        family: "Motorola device timestamps",
        strategy: Strategy::Packed(decode_moto),
        citation: "Motorola 6-byte (one byte per field, year+1970); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "symantec",
        label: "Symantec AV 6-byte timestamp",
        family: "Symantec antivirus logs",
        strategy: Strategy::Packed(decode_symantec),
        citation: "Symantec AV 6-byte (year+1970, month+1); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "dvr",
        label: "DVR (WFS/DHFS) packed timestamp (LOCAL time)",
        family: "DVR WFS / DHFS filesystems",
        strategy: Strategy::Packed(decode_dvr),
        citation: "DVR WFS/DHFS 32-bit packed (year since 2000); vs time-decode",
        tz: LocalNaive,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "sony",
        label: "Sonyflake ID (10ms units since 2014-09-01, <<24)",
        family: "Sonyflake distributed IDs",
        strategy: Strategy::Embedded {
            epoch_ns: SONY_EPOCH_NS,
            shift_bits: 24,
            unit: Unit::CentiSecond,
        },
        citation: "Sonyflake (id>>24 in 10ms units, 2014-09-01 epoch); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "ns40",
        label: "Nokia S40 7-byte timestamp",
        family: "Nokia S40 devices",
        strategy: Strategy::Packed(decode_ns40),
        citation: "Nokia S40 7-byte (year BE u16 + field bytes); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "ns40le",
        label: "Nokia S40 7-byte timestamp (LE year)",
        family: "Nokia S40 devices",
        strategy: Strategy::Packed(decode_ns40le),
        citation: "Nokia S40 7-byte, little-endian year u16; vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "logtime",
        label: "JET LogTime 8-byte timestamp",
        family: "Microsoft JET / ESE database logs",
        strategy: Strategy::Packed(decode_logtime),
        citation: "JET LogTime (reversed field bytes, year+1900); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "semioctet",
        label: "Semi-Octet decimal (LOCAL time)",
        family: "Semi-octet (nibble-swapped) timestamps",
        strategy: Strategy::Packed(decode_semioctet),
        citation: "Semi-Octet decimal (nibble-swapped pairs, YY+2000); vs time-decode",
        tz: LocalNaive,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "gsm",
        label: "GSM 7-byte semi-octet timestamp",
        family: "GSM mobile timestamps",
        strategy: Strategy::Packed(decode_gsm),
        citation: "GSM semi-octet (per-byte nibble swap + tz byte); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
    Format {
        id: "nokiale",
        label: "Nokia time LE (seconds before 2050)",
        family: "Nokia devices",
        strategy: Strategy::Packed(decode_nokiale),
        citation: "Nokia LE (byte-reversed two's-complement seconds before 2050); vs time-decode",
        tz: Utc,
        leap: PosixIgnored,
        plausible: W,
    },
];
