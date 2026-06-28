//! `timeglyph` — forensic timestamp decipherment.
//!
//! A timestamp is *time inscribed as a symbol* — the raw integer or bytes a
//! system writes to mean an instant. This crate deciphers those inscriptions:
//! it decodes a known format to an instant, encodes an instant to any format,
//! and — the differentiator — **identifies** an unknown value by reporting every
//! plausible interpretation, *scored, with stated assumptions*, never "the
//! answer" (a single integer is usually underdetermined).
//!
//! # Design (see HANDOFF.md for the full record)
//! - Canonical spine: [`PosixNs`] — nanoseconds since the Unix epoch, proleptic
//!   Gregorian, **leap-second-ignoring (POSIX)**. It is *not* called UTC: UTC has
//!   discontinuities POSIX pretends away. Leap-aware scales (TAI/GPS/NTP) get
//!   their own instant types (to be added behind a `hifitime` feature).
//! - Calendar/tz math is **reused** (`jiff`), never reinvented. The value-add is
//!   the cited forensic format registry + scored auto-detection + byte decode.
//! - Panic-free (Paranoid Gatekeeper): every length/offset/width is checked.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod interpret;
pub mod registry;

/// Errors from decoding, encoding, or rendering a timestamp.
#[derive(Debug, thiserror::Error)]
pub enum ChronoError {
    /// A value (or intermediate) fell outside the representable range.
    #[error("value out of representable range ({what}): {value}")]
    OutOfRange {
        /// What overflowed (e.g. "nanoseconds", "ticks").
        what: &'static str,
        /// The offending value.
        value: i128,
    },
    /// No format with the given id is registered.
    #[error("unknown format id: {0}")]
    UnknownFormat(String),
    /// Rendering the instant to a civil string failed (outside jiff's range).
    #[error("cannot render instant: {0}")]
    Render(String),
}

/// The canonical internal instant: **nanoseconds since 1970-01-01, POSIX
/// (leap-ignoring), proleptic Gregorian**. `i128` because some source epochs sit
/// >1e19 ns from Unix (FILETIME's 1601 epoch alone is ~1.16e19 ns), which
/// overflows `i64` — the wide spine is load-bearing, not luxury.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct PosixNs(pub i128);

impl PosixNs {
    /// The Unix epoch (the zero of this scale).
    pub const UNIX_EPOCH: Self = Self(0);

    /// Render as an RFC 3339 / ISO 8601 UTC string. Returns `None` when the
    /// instant is outside the civil range `jiff` can represent (≈ years
    /// -9999..=9999) — surfaced as absence, never a panic.
    #[must_use]
    pub fn to_rfc3339(self) -> Option<String> {
        jiff::Timestamp::from_nanosecond(self.0)
            .ok()
            .map(|ts| ts.to_string())
    }
}

/// The tick unit a format counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Whole seconds.
    Seconds,
    /// Milliseconds (Java/JS).
    Millis,
    /// Microseconds (Chrome/WebKit, PostgreSQL).
    Micros,
    /// 100-nanosecond intervals (FILETIME, .NET ticks).
    HundredNanos,
    /// Nanoseconds (APFS, Unix-ns).
    Nanos,
    /// Whole days (OLE Automation / Excel serial — usually fractional).
    Days,
}

impl Unit {
    /// Nanoseconds per tick of this unit.
    #[must_use]
    pub const fn nanos(self) -> i128 {
        match self {
            Self::Seconds => 1_000_000_000,
            Self::Millis => 1_000_000,
            Self::Micros => 1_000,
            Self::HundredNanos => 100,
            Self::Nanos => 1,
            Self::Days => 86_400 * 1_000_000_000,
        }
    }
}

/// How a stored value maps to an instant.
#[derive(Debug, Clone, Copy)]
pub enum Strategy {
    /// `value` (integer ticks) × `unit` + `epoch_ns` = [`PosixNs`].
    LinearInt {
        /// The format's epoch as nanoseconds relative to the Unix epoch.
        epoch_ns: i128,
        /// The tick unit.
        unit: Unit,
    },
    /// `value` (floating ticks, e.g. OLE days as `f64`) × `unit` + `epoch_ns`.
    /// Lossy by nature; the registry entry must flag the precision caveat.
    LinearFloat {
        /// The format's epoch as nanoseconds relative to the Unix epoch.
        epoch_ns: i128,
        /// The tick unit.
        unit: Unit,
    },
    // TODO(HANDOFF): Packed(fn) for FAT/DOS/SYSTEMTIME/exFAT bit-packed structs;
    // Snowflake/ObjectId/UUIDv7 (embedded-ms with bit shifts); ASN.1 string forms.
}

/// Timezone semantics of a format's stored value — NOT garnish: FAT stores local
/// time, EXIF often lacks an offset, Event Logs store UTC but display local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TzSemantics {
    /// The value denotes UTC (POSIX, leap-ignoring).
    Utc,
    /// The value denotes naive *local* time with no recorded offset (FAT/DOS).
    LocalNaive,
    /// The value carries its own offset (exFAT tz field, EXIF with offset).
    OffsetEmbedded,
}

/// Leap-second semantics — the partition Codex flagged. Most forensic epochs are
/// POSIX (leap-ignoring); only the GPS/TAI/NTP family needs true leap math.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeapSemantics {
    /// UTC-labelled but leap-ignoring (pure constant offset to Unix). The norm.
    PosixIgnored,
    /// True leap-aware scale (GPS/TAI/NTP) — handled by a separate instant type.
    LeapAware,
}

/// One forensic timestamp format: evidence metadata, not just a converter.
#[derive(Debug, Clone, Copy)]
pub struct Format {
    /// Stable id (e.g. `"filetime"`).
    pub id: &'static str,
    /// Human label (e.g. `"Windows FILETIME"`).
    pub label: &'static str,
    /// Where it's found / who writes it.
    pub family: &'static str,
    /// How the value maps to an instant.
    pub strategy: Strategy,
    /// Authoritative spec citation (clean-room provenance for the paper).
    pub citation: &'static str,
    /// Timezone semantics.
    pub tz: TzSemantics,
    /// Leap-second semantics.
    pub leap: LeapSemantics,
    /// Observed forensic plausibility window `[from, to)` in [`PosixNs`] — used
    /// to rank auto-detect candidates (NOT to assert a single answer).
    pub plausible: (i128, i128),
}

impl Format {
    /// Decode an integer value under this format. Errors (never panics) on
    /// overflow or on a float-only strategy.
    pub fn decode_int(&self, value: i64) -> Result<PosixNs, ChronoError> {
        match self.strategy {
            Strategy::LinearInt { epoch_ns, unit } => {
                let ticks = i128::from(value);
                let ns = ticks
                    .checked_mul(unit.nanos())
                    .and_then(|t| t.checked_add(epoch_ns))
                    .ok_or(ChronoError::OutOfRange {
                        what: "nanoseconds",
                        value: ticks,
                    })?;
                Ok(PosixNs(ns))
            }
            Strategy::LinearFloat { .. } => Err(ChronoError::OutOfRange {
                what: "float-format decoded as integer",
                value: i128::from(value),
            }),
        }
    }

    /// Decode a floating value (OLE days etc.). Lossy; see `precision` caveat.
    pub fn decode_float(&self, value: f64) -> Result<PosixNs, ChronoError> {
        match self.strategy {
            Strategy::LinearFloat { epoch_ns, unit } => {
                // f64 days × ns/day, then the epoch offset. Precision loss is
                // inherent to the source encoding (documented, not hidden).
                let ns = (value * unit.nanos() as f64).round() as i128;
                Ok(PosixNs(ns + epoch_ns))
            }
            Strategy::LinearInt { .. } => Err(ChronoError::OutOfRange {
                what: "integer format decoded as float",
                value: 0,
            }),
        }
    }

    /// Encode an instant to this format's integer value (truncating toward the
    /// epoch at the unit granularity). Errors on overflow / float-only formats.
    pub fn encode_int(&self, instant: PosixNs) -> Result<i64, ChronoError> {
        match self.strategy {
            Strategy::LinearInt { epoch_ns, unit } => {
                let rel = instant
                    .0
                    .checked_sub(epoch_ns)
                    .ok_or(ChronoError::OutOfRange {
                        what: "nanoseconds",
                        value: instant.0,
                    })?;
                let ticks = rel / unit.nanos();
                i64::try_from(ticks).map_err(|_| ChronoError::OutOfRange {
                    what: "ticks",
                    value: ticks,
                })
            }
            Strategy::LinearFloat { .. } => Err(ChronoError::OutOfRange {
                what: "float-format encoded as integer",
                value: 0,
            }),
        }
    }
}

/// Look up a registered format by id.
pub fn format(id: &str) -> Result<&'static Format, ChronoError> {
    registry::FORMATS
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(|| ChronoError::UnknownFormat(id.to_string()))
}
