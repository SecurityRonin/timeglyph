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
//!
//! # Example
//!
//! ```
//! // Identify an unknown value: every plausible reading, ranked and scored —
//! // never a single verdict (a raw value is usually underdetermined).
//! let candidates = timeglyph::interpret::interpret_int(1_577_836_800);
//! let top = &candidates[0];
//! assert_eq!(top.format_id, "unix");
//! assert_eq!(top.rendered.as_deref(), Some("2020-01-01T00:00:00Z"));
//!
//! // Or decode under one known format by id.
//! let filetime = timeglyph::format("filetime").unwrap();
//! let instant = filetime.decode_int(132_223_104_000_000_000).unwrap();
//! assert_eq!(instant.to_rfc3339().as_deref(), Some("2020-01-01T00:00:00Z"));
//! ```
//!
//! # Further reading
//!
//! The authoritative, primary-source-cited reference for every supported format —
//! epochs, encodings, calendars, leap seconds, and the rollovers that eventually
//! break them — lives at <https://securityronin.github.io/timeglyph/>.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod csv_enrich;
pub mod interpret;
/// Leap-aware time scales (GPS/TAI/NTP), behind the `leap` feature. Kept
/// separate from the POSIX [`PosixNs`] spine (HANDOFF §3).
#[cfg(feature = "leap")]
pub mod leap;
/// Chinese lunisolar calendar + 干支 four-pillar rendering, behind the
/// `lunisolar` feature. Convention-relative: needs a meridian (and optional
/// longitude), unlike the instant↔instant rest of the crate.
#[cfg(feature = "lunisolar")]
pub mod lunisolar;
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
    /// The requested output timezone is neither UTC, a valid fixed offset, nor a
    /// known IANA zone name. Surfaced (never a silent UTC fallback) so the
    /// rendered offset is always the one the analyst asked for.
    #[error("unknown timezone: {0} (expected UTC, a fixed offset like +08:00, or an IANA name like America/New_York)")]
    UnknownZone(String),
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

    /// Render this instant in a chosen output [`RenderZone`]. The instant is
    /// absolute; the zone changes only the *displayed* civil time and offset.
    /// Returns `None` only when the instant is outside the civil range (same
    /// contract as [`to_rfc3339`](Self::to_rfc3339)). For a named zone the
    /// offset is resolved per-instant, so DST is handled correctly.
    #[must_use]
    pub fn render(self, zone: &RenderZone) -> Option<String> {
        let ts = jiff::Timestamp::from_nanosecond(self.0).ok()?;
        match zone {
            // UTC keeps the `Z` suffix: "the offset to local time is unknown"
            // is the honest default for an instant with no recorded locale.
            RenderZone::Utc => Some(ts.to_string()),
            RenderZone::Fixed(offset) => Some(ts.display_with_offset(*offset).to_string()),
            RenderZone::Named(tz) => {
                let offset = tz.to_offset(ts);
                Some(ts.display_with_offset(offset).to_string())
            }
        }
    }
}

/// A target timezone for *rendering* an instant ([`PosixNs`]). Presentation
/// only — it never changes the underlying instant, just how it is displayed.
/// The default ([`RenderZone::Utc`]) renders with a `Z` suffix.
#[derive(Debug, Clone)]
pub enum RenderZone {
    /// UTC, rendered with a `Z` suffix (the unambiguous default).
    Utc,
    /// A fixed offset from UTC (e.g. `+08:00`), rendered with a numeric offset.
    Fixed(jiff::tz::Offset),
    /// A named IANA zone (e.g. `America/New_York`), pre-validated at parse time.
    /// The offset is resolved per instant, so the rendering is DST-correct.
    Named(jiff::tz::TimeZone),
}

impl RenderZone {
    /// Parse a zone spec: empty / `UTC` / `Z` → UTC; a leading `+`/`-` → a fixed
    /// offset (`+HH`, `±HH:MM`, `±HHMM`); anything else → an IANA zone name,
    /// validated against the tz database. An unrecognised name errors loudly
    /// ([`ChronoError::UnknownZone`]) rather than silently falling back to UTC.
    pub fn parse(spec: &str) -> Result<Self, ChronoError> {
        let s = spec.trim();
        if s.is_empty() || s.eq_ignore_ascii_case("utc") || s.eq_ignore_ascii_case("z") {
            return Ok(Self::Utc);
        }
        if matches!(s.as_bytes().first(), Some(b'+' | b'-')) {
            return parse_offset(s)
                .map(Self::Fixed)
                .ok_or_else(|| ChronoError::UnknownZone(s.to_string()));
        }
        jiff::tz::TimeZone::get(s)
            .map(Self::Named)
            .map_err(|_| ChronoError::UnknownZone(s.to_string()))
    }
}

/// Parse a fixed UTC offset of the form `±HH`, `±HH:MM`, or `±HHMM` into a jiff
/// [`Offset`](jiff::tz::Offset). Returns `None` on a malformed or out-of-range
/// offset (never a fabricated zero) so the caller can reject it.
fn parse_offset(s: &str) -> Option<jiff::tz::Offset> {
    let (sign, rest) = match s.as_bytes().first()? {
        b'+' => (1i32, &s[1..]),
        b'-' => (-1i32, &s[1..]),
        _ => return None,
    };
    let digits: String = rest.chars().filter(|c| *c != ':').collect();
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (hh, mm) = match digits.len() {
        1 | 2 => (digits.parse::<i32>().ok()?, 0),
        4 => (digits[..2].parse().ok()?, digits[2..].parse::<i32>().ok()?),
        _ => return None,
    };
    if hh > 23 || mm > 59 {
        return None;
    }
    jiff::tz::Offset::from_seconds(sign * (hh * 3600 + mm * 60)).ok()
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

    /// Decimal digits of *sub-second* resolution this unit can express
    /// (seconds/days → 0, millis → 3, micros → 6, 100-nanos → 7, nanos → 9).
    /// Drives auto-detect granularity scoring: a whole-second raw value is a
    /// poor fit for a finer unit, so it is penalised, never hidden.
    #[must_use]
    pub const fn sub_second_digits(self) -> u32 {
        match self {
            Self::Seconds | Self::Days => 0,
            Self::Millis => 3,
            Self::Micros => 6,
            Self::HundredNanos => 7,
            Self::Nanos => 9,
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
    /// An ID with an embedded timestamp in its high bits: the low `shift_bits`
    /// bits are worker/sequence/random, so `value >> shift_bits` is a count of
    /// `unit` ticks since `epoch_ns`. Most snowflake-class IDs count
    /// milliseconds (Twitter/Discord/Mastodon/LinkedIn), but the unit is part of
    /// the scheme — TikTok counts whole seconds — so it is carried explicitly.
    Embedded {
        /// The scheme's epoch as nanoseconds relative to the Unix epoch.
        epoch_ns: i128,
        /// Number of low bits to discard before reading the timestamp.
        shift_bits: u32,
        /// The tick unit of the embedded timestamp.
        unit: Unit,
    },
    /// A bit-packed civil datetime (FAT/DOS, SYSTEMTIME, exFAT): the integer is
    /// not a linear offset but packed calendar fields, so decoding needs a
    /// dedicated unpacker. The function returns the instant; tz semantics (e.g.
    /// FAT's LOCAL naive time) are carried on the [`Format`] entry.
    Packed(fn(i64) -> Result<PosixNs, ChronoError>),
    // TODO(HANDOFF): SYSTEMTIME / exFAT (offset field) packed layouts;
    // ASN.1 / EXIF / RFC-2822 string forms.
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
    /// The natural on-disk storage width, in bytes, of this format's stored
    /// value. A structural prior for byte-width scoring (HANDOFF §5b), NOT a hard
    /// rule: second/day-resolution fields are classically 32-bit (Unix `time_t`,
    /// HFS+, DOS date words), while sub-second and ID fields are 64-bit
    /// (FILETIME, .NET ticks, ms/µs/ns counts, snowflake IDs, OLE `f64`).
    #[must_use]
    pub fn storage_bytes(&self) -> u8 {
        match self.strategy {
            Strategy::Packed(_) => 4,
            Strategy::Embedded { .. } | Strategy::LinearFloat { .. } => 8,
            Strategy::LinearInt { unit, .. } => match unit {
                Unit::Seconds | Unit::Days => 4,
                Unit::Millis | Unit::Micros | Unit::HundredNanos | Unit::Nanos => 8,
            },
        }
    }

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
            Strategy::Embedded {
                epoch_ns,
                shift_bits,
                unit,
            } => {
                // IDs are unsigned; a negative value is not a valid ID encoding.
                let raw = u64::try_from(value).map_err(|_| ChronoError::OutOfRange {
                    what: "embedded-id (negative)",
                    value: i128::from(value),
                })?;
                let ticks = i128::from(raw >> shift_bits);
                let ns = ticks
                    .checked_mul(unit.nanos())
                    .and_then(|t| t.checked_add(epoch_ns))
                    .ok_or(ChronoError::OutOfRange {
                        what: "nanoseconds",
                        value: ticks,
                    })?;
                Ok(PosixNs(ns))
            }
            Strategy::Packed(decode) => decode(value),
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
                // Reject non-finite or absurd magnitudes rather than let the
                // float→int cast saturate into a plausible-but-wrong instant.
                if !value.is_finite() {
                    return Err(ChronoError::OutOfRange {
                        what: "non-finite float value",
                        value: 0,
                    });
                }
                let scaled = (value * unit.nanos() as f64).round();
                // 1e38 < i128::MAX (~1.7e38): a safe ceiling below the saturating
                // cast boundary, well past any civil-range date.
                if !scaled.is_finite() || scaled.abs() >= 1.0e38 {
                    return Err(ChronoError::OutOfRange {
                        what: "float value out of representable range",
                        value: 0,
                    });
                }
                let ns = (scaled as i128)
                    .checked_add(epoch_ns)
                    .ok_or(ChronoError::OutOfRange {
                        what: "nanoseconds",
                        value: scaled as i128,
                    })?;
                Ok(PosixNs(ns))
            }
            Strategy::LinearInt { .. } | Strategy::Embedded { .. } | Strategy::Packed(_) => {
                Err(ChronoError::OutOfRange {
                    what: "integer format decoded as float",
                    value: 0,
                })
            }
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
            Strategy::Embedded { .. } => Err(ChronoError::OutOfRange {
                // Encoding would have to invent the worker/sequence low bits; a
                // round-trip is not defined for ID schemes.
                what: "embedded-id format cannot be re-encoded from an instant",
                value: 0,
            }),
            Strategy::Packed(_) => Err(ChronoError::OutOfRange {
                what: "packed format cannot be re-encoded from an instant",
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
