//! `timeglyph` — forensic timestamp decipherment.
//!
//! A timestamp is *time inscribed as a symbol* — the raw integer or bytes a
//! system writes to mean an instant. This crate deciphers those inscriptions:
//! it decodes a known format to an instant, encodes an instant to any format,
//! and — the differentiator — **identifies** an unknown value by reporting every
//! plausible interpretation, *scored, with stated assumptions*, never "the
//! answer" (a single integer is usually underdetermined).
//!
//! # Design (see docs/decisions/ for the ADRs)
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

/// Bounded carve: find timestamps at every offset of a raw byte blob
/// (`identify_bytes` swept over offsets, window + score-thresholded).
/// Reference calendar: civil facts of a date (ISO week, day-of-year, JDN/MJD,
/// Unix midnight, weekday), the base of the `cal` subcommand.
pub mod cal;
/// ASCII art (moon discs, seasonal tiles) for the `cal` visual layer.
#[cfg(feature = "lunisolar")]
pub mod cal_art;
/// Terminal colour (truecolor→256→16→mono ladder) for the `cal` visual layer.
pub mod cal_color;
/// Pure text renderers for the `cal` month grid.
pub mod cal_render;
/// Shared calendar-display formatters (Chinese lunar date / 干支 / solar term,
/// Hebrew & Islamic month names, 五行) — the DRY source for `cal` and the lens.
pub mod calfmt;
pub mod carve;
pub mod compose;
#[cfg(feature = "csv")]
pub mod csv_enrich;
pub mod datefmt;
pub use datefmt::DateStyle;
/// Whole-world public-holiday lookup (ISO-3166 country + date → holiday name),
/// behind the `holiday` feature. An embedded python-holidays export; a hit is
/// "consistent with a public holiday", an annotation rather than a guarantee.
#[cfg(feature = "holiday")]
pub mod holiday;
pub mod interpret;
/// Resolve a `LocalNaive` wall-clock value *in* a concrete zone — DST fold/gap
/// (correctness wave). Pure over the IANA tzdb.
pub mod localzone;
/// MCP (Model Context Protocol) stdio JSON-RPC handler — expose the engine as
/// tools for LLM-driven DFIR (cited readings, not hallucinated epoch math).
pub mod mcp;
pub use localzone::{resolve_local, LocalResolution};
/// Leap-aware time scales (GPS/TAI/NTP), behind the `leap` feature. Kept
/// separate from the POSIX [`PosixNs`] spine (ADR 0003).
#[cfg(feature = "leap")]
pub mod leap;
/// Chinese lunisolar calendar + 干支 four-pillar rendering, behind the
/// `lunisolar` feature. Convention-relative: needs a meridian (and optional
/// longitude), unlike the instant↔instant rest of the crate.
#[cfg(feature = "lunisolar")]
pub mod lunisolar;
pub mod registry;
/// Scan arbitrary text for timestamp candidates and decode each into ranked
/// readings (the CLI `scan` command and the timeglyph-lens overlay share this).
pub mod scan;
/// Whole-second convenience over the `PosixNs` spine (filesystem/bodyfile use).
pub mod secs;

/// The engine's version (`CARGO_PKG_VERSION`), for callers that surface it — e.g.
/// the timeglyph-lens overlay's landing screen.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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

    /// Whole Unix seconds, floored toward negative infinity (sub-second precision
    /// dropped). For callers that store a plain `i64` seconds timestamp — see the
    /// [`crate::secs`] convenience module.
    #[must_use]
    pub fn unix_seconds(self) -> i64 {
        self.0.div_euclid(1_000_000_000) as i64
    }

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
        // cov:unreachable: the only caller (`RenderZone::parse`) enters this
        // function behind `matches!(s.as_bytes().first(), Some(b'+' | b'-'))`, so
        // an unsigned spec never arrives. Kept so a second caller cannot smuggle
        // one past the sign check.
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

// The timestamp-format *knowledge* — tick units, timezone/leap semantics, the
// packed-layout tags, the `Encoding` a format uses, and the [`TimeFormat`] record
// itself — lives in forensicnomicon (the zero-dep DFIR knowledge leaf). timeglyph
// is the *engine* that decodes it, so it re-exports those types and wraps each
// catalog entry in an engine-side [`Format`] that adds the decode/encode methods
// and the packed codec.
pub use forensicnomicon::temporal_formats::{
    Encoding, LeapSemantics, PackedLayout, TimeFormat, TzSemantics, Unit,
};

/// The engine-side codec for a [`Encoding::Packed`] format: the layout-specific
/// unpacker (and inverse packer, when one exists) that the knowledge table's
/// [`PackedLayout`] tag dispatches to. The calendar math lives here in the engine,
/// not in the zero-dep knowledge table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PackedCodec {
    /// Unpack the integer into an instant.
    pub(crate) decode: fn(i64) -> Result<PosixNs, ChronoError>,
    /// Inverse packer, if the format can re-encode an instant to its packed
    /// integer. `None` until an oracle-validated encoder exists for it.
    pub(crate) encode: Option<fn(PosixNs) -> Result<i64, ChronoError>>,
}

/// One forensic timestamp format: the authoritative catalog record ([`meta`], the
/// evidence metadata + [`Encoding`], owned by forensicnomicon) plus the engine's
/// packed codec when the encoding is [`Encoding::Packed`]. Derefs to [`meta`], so
/// `f.id`, `f.citation`, `f.tz`, `f.encoding`, … read straight through to the
/// catalog entry.
///
/// [`meta`]: Format::meta
#[derive(Debug, Clone, Copy)]
pub struct Format {
    /// The authoritative catalog record from forensicnomicon (id, label, family,
    /// citation, tz/leap semantics, plausibility window, and the [`Encoding`]).
    pub meta: &'static TimeFormat,
    /// The engine codec for a packed encoding; `None` for linear/embedded/float.
    pub(crate) packed: Option<PackedCodec>,
}

impl std::ops::Deref for Format {
    type Target = TimeFormat;
    fn deref(&self) -> &Self::Target {
        self.meta
    }
}

impl Format {
    /// The natural on-disk storage width, in bytes, of this format's stored
    /// value. A structural prior for byte-width scoring (ADR 0005), NOT a hard
    /// rule: second/day-resolution fields are classically 32-bit (Unix `time_t`,
    /// HFS+, DOS date words), while sub-second and ID fields are 64-bit
    /// (FILETIME, .NET ticks, ms/µs/ns counts, snowflake IDs, OLE `f64`).
    #[must_use]
    pub fn storage_bytes(&self) -> u8 {
        match self.meta.encoding {
            Encoding::Packed(_) => 4,
            Encoding::Embedded { .. } | Encoding::LinearFloat { .. } => 8,
            Encoding::LinearInt { unit, .. } => match unit {
                Unit::Seconds | Unit::Days => 4,
                Unit::CentiSecond
                | Unit::Millis
                | Unit::Micros
                | Unit::HundredNanos
                | Unit::Nanos => 8,
            },
        }
    }

    /// Decode an integer value under this format. Errors (never panics) on
    /// overflow or on a float-only strategy.
    pub fn decode_int(&self, value: i64) -> Result<PosixNs, ChronoError> {
        match self.meta.encoding {
            Encoding::LinearInt { epoch_ns, unit } => {
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
            Encoding::Embedded {
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
            Encoding::Packed(_) => (self.packed_codec()?.decode)(value),
            Encoding::LinearFloat { .. } => Err(ChronoError::OutOfRange {
                what: "float-format decoded as integer",
                value: i128::from(value),
            }),
        }
    }

    /// The engine codec for this format's packed layout, or a loud error if the
    /// encoding is `Packed` but no codec was wired (an internal invariant break —
    /// the registry builder attaches a codec to every packed format).
    fn packed_codec(&self) -> Result<PackedCodec, ChronoError> {
        self.packed.ok_or(ChronoError::OutOfRange {
            what: "packed format has no engine codec (internal invariant broken)",
            value: 0,
        })
    }

    /// Decode a floating value (OLE days etc.). Lossy; see `precision` caveat.
    pub fn decode_float(&self, value: f64) -> Result<PosixNs, ChronoError> {
        match self.meta.encoding {
            Encoding::LinearFloat { epoch_ns, unit } => {
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
            Encoding::LinearInt { .. } | Encoding::Embedded { .. } | Encoding::Packed(_) => {
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
        match self.meta.encoding {
            Encoding::LinearInt { epoch_ns, unit } => {
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
            Encoding::LinearFloat { .. } => Err(ChronoError::OutOfRange {
                what: "float-format encoded as integer",
                value: 0,
            }),
            Encoding::Embedded {
                epoch_ns,
                shift_bits,
                unit,
            } => {
                // Canonical ID: the timestamp in the high bits, worker/sequence
                // low bits zeroed. decode() discards the low bits, so the instant
                // round-trips; the low bits are not reconstructible from an instant
                // and are legitimately zero (the earliest ID at this instant).
                let rel = instant
                    .0
                    .checked_sub(epoch_ns)
                    .ok_or(ChronoError::OutOfRange {
                        what: "nanoseconds",
                        value: instant.0,
                    })?;
                let shifted = (rel / unit.nanos()) << shift_bits;
                i64::try_from(shifted).map_err(|_| ChronoError::OutOfRange {
                    what: "embedded-id ticks",
                    value: shifted,
                })
            }
            Encoding::Packed(_) => match self.packed_codec()?.encode {
                Some(f) => f(instant),
                // cov:unreachable: every `PackedCodec` in the registry today sets
                // `encode: Some(..)` (all 16 packed codecs are oracle-validated in
                // both directions), so this arm is dead until a decode-only format
                // is added — which is exactly what it exists to report.
                None => Err(ChronoError::OutOfRange {
                    what: "packed format cannot yet be re-encoded from an instant",
                    value: 0,
                }),
            },
        }
    }

    /// Encode an instant to this format's floating-point value — LinearFloat
    /// formats only (OLE, SQLite Julian, Excel, Cocoa double). Errors for
    /// integer / embedded / packed strategies (use [`Format::encode_int`]).
    pub fn encode_float(&self, instant: PosixNs) -> Result<f64, ChronoError> {
        match self.meta.encoding {
            Encoding::LinearFloat { epoch_ns, unit } => {
                let rel = instant.0 - epoch_ns;
                Ok(rel as f64 / unit.nanos() as f64)
            }
            _ => Err(ChronoError::OutOfRange {
                what: "non-float format encoded as a float",
                value: 0,
            }),
        }
    }

    /// Encode an instant to this format's natural value: an integer for linear /
    /// embedded / packed formats, a float for float formats.
    pub fn encode(&self, instant: PosixNs) -> Result<Encoded, ChronoError> {
        match self.meta.encoding {
            Encoding::LinearFloat { .. } => self.encode_float(instant).map(Encoded::Float),
            _ => self.encode_int(instant).map(Encoded::Int),
        }
    }
}

/// The natural encoded value of a format: an integer (linear / embedded / packed)
/// or a float (OLE / Julian / Excel / Cocoa double). Displays as the bare value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Encoded {
    /// An integer-valued encoding.
    Int(i64),
    /// A floating-point encoding.
    Float(f64),
}

impl std::fmt::Display for Encoded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Encoded::Int(i) => write!(f, "{i}"),
            Encoded::Float(x) => write!(f, "{x}"),
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

/// A deterministic 16-hex-character fingerprint of the format registry — each
/// entry's `id` and `citation`, in catalog order. The provenance anchor for
/// `--provenance`: the same engine build always yields the same digest, and any
/// change to a format definition changes it, so a reading is traceable to the
/// exact method version that produced it. Pure FNV-1a (no dependency). Iterates
/// the authoritative forensicnomicon catalog (the source the engine wraps).
#[must_use]
pub fn registry_digest() -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a 64-bit offset basis
    for f in forensicnomicon::temporal_formats::TIME_FORMATS {
        for byte in f.id.bytes().chain(f.citation.bytes()) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
        }
    }
    format!("{hash:016x}")
}
