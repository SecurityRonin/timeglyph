//! The forensic format registry.
//!
//! SCAFFOLD: a handful of exemplar formats establishing the pattern (LinearInt /
//! LinearFloat, epoch_ns constants, citations, plausibility windows). The full
//! ~70-format catalog — and the Packed/embedded strategies (FAT/DOS, SYSTEMTIME,
//! Snowflake, ObjectId, UUIDv7, ASN.1 string forms) — is the build-out work; see
//! HANDOFF.md §"Format catalog TODO" for the complete list + spec citations.
//!
//! Every epoch_ns constant below is a CLEAN-ROOM fact from a primary spec, to be
//! cross-validated against the MIT `time_decode` oracle and each spec's worked
//! example (HANDOFF §"Validation"). NEVER sourced from decompiling DCode.

use crate::{Format, LeapSemantics::PosixIgnored, Strategy, TzSemantics::Utc, Unit};

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
];
