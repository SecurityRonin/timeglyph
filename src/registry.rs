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
];
