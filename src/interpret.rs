//! Auto-detection: identify an unknown value by reporting EVERY plausible
//! interpretation, **scored, with stated assumptions** — never "the detected
//! format." A single integer is usually underdetermined: a 64-bit value can be a
//! plausible Unix-s, Java-ms, Chrome-µs, FILETIME, .NET-ticks and Cocoa-s date
//! all at once. Presenting one as *the* answer would fabricate certainty, which a
//! forensic tool must never do (epistemics: "consistent with", not a verdict).
//!
//! SCAFFOLD scoring is deliberately thin (plausibility-window membership only).
//! HANDOFF §"Plausibility scoring" lists the full component set to implement:
//! representable validity, configured-case-window, granularity match, byte-width
//! match, endian match, artifact-context hint, neighbour-monotonicity.

use crate::{
    registry::FORMATS, ChronoError, Format, LeapSemantics, PosixNs, Strategy, TzSemantics, Unit,
};

/// One candidate interpretation of a value. Carries its score *components* and
/// *assumptions*, not just a rank — transparency over false confidence.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Candidate {
    /// Format id (e.g. `"filetime"`).
    pub format_id: &'static str,
    /// Human label.
    pub label: &'static str,
    /// Spec citation for the assumed format.
    pub citation: &'static str,
    /// The decoded instant.
    pub instant: PosixNs,
    /// RFC 3339 rendering, or `None` if outside the civil range.
    pub rendered: Option<String>,
    /// Overall plausibility score in `[0, 1]` (scaffold: window membership).
    pub score: f64,
    /// The individual scored components (named), for auditability.
    pub components: Vec<(&'static str, f64)>,
    /// Assumptions made to produce this reading (e.g. the format + citation).
    pub assumptions: Vec<String>,
    /// True when the raw value is a well-known "magic" sentinel (0/unset, −1,
    /// `i64::MAX`/never) rather than a real instant. Machine-readable so pipelines
    /// can refuse to treat it as authoritative (see also [`Candidate::score`]).
    pub sentinel: bool,
}

/// Interpret a raw integer across every integer-decodable format (linear,
/// embedded-millisecond IDs, and packed). Returns ALL readings that render to a
/// civil date, ranked by score (descending), then by id for determinism. The
/// caller MUST present these as candidates, not a single answer.
///
/// ```
/// let candidates = timeglyph::interpret::interpret_int(1_577_836_800);
/// // A raw value is underdetermined — expect several plausible readings.
/// assert!(candidates.len() >= 2);
/// // The top-ranked reading carries its scored components and assumptions.
/// let top = &candidates[0];
/// assert!(top.components.iter().any(|(name, _)| *name == "granularity_match"));
/// assert!(!top.assumptions.is_empty());
/// ```
#[must_use]
pub fn interpret_int(value: i64) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for f in FORMATS {
        // Any integer-decodable strategy is a candidate; float-only and
        // out-of-range readings are skipped inside build_candidate.
        if let Some(c) = build_candidate(f, value) {
            out.push(c);
        }
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.format_id.cmp(b.format_id))
    });
    out
}

/// Build a scored, assumption-carrying candidate for one format + integer value,
/// or `None` if the value is not integer-decodable under it or renders outside the
/// civil range. Shared by [`interpret_int`] and the per-format hex decoders.
fn build_candidate(f: &Format, value: i64) -> Option<Candidate> {
    let instant = f.decode_int(value).ok()?;
    let rendered = instant.to_rfc3339()?;
    let components = score_components(f, value, instant);
    let score = overall_score(&components);
    let mut assumptions = assumptions(f);
    let sentinel = sentinel_reason(value);
    if let Some(reason) = sentinel {
        assumptions.push(format!(
            "value {value} is a likely sentinel ({reason}) — an 'unset'/'never' marker, not necessarily a real instant"
        ));
    }
    Some(Candidate {
        format_id: f.id,
        label: f.label,
        citation: f.citation,
        instant,
        rendered: Some(rendered),
        score,
        components,
        assumptions,
        sentinel: sentinel.is_some(),
    })
}

/// Decode a single named format from an integer value, returning its candidate.
fn decode_one(format_id: &str, value: i64) -> Option<Candidate> {
    build_candidate(crate::format(format_id).ok()?, value)
}

/// The stated assumptions behind one reading (HANDOFF §5c epistemics). A reading
/// is evidence, not a verdict: it is framed as *consistent with* a format, never
/// "detected". POSIX-labelled readings additionally carry the leap-smear
/// disclaimer — a raw value cannot reveal whether its source clock smeared leap
/// seconds (Google/AWS/Meta smear is invisible without clock-policy metadata).
fn assumptions(f: &Format) -> Vec<String> {
    let mut out = vec![format!(
        "consistent with {} [{}] — a reading, not a determination",
        f.label, f.citation
    )];
    if matches!(f.leap, LeapSemantics::PosixIgnored) {
        out.push(
            "indistinguishable from a leap-smeared source without clock-policy metadata"
                .to_string(),
        );
    }
    if matches!(f.tz, TzSemantics::LocalNaive) {
        out.push(
            "stored as LOCAL wall-clock time with no offset — the instant is naive, not UTC"
                .to_string(),
        );
    }
    out
}

/// Well-known "magic" sentinel values that denote unset/never/error rather than
/// a real instant (a zero/uninitialized field, an all-ones marker, the Active
/// Directory `accountExpires = 0x7FFFFFFFFFFFFFFF` "never"). Detecting them is the
/// front line against silently rendering a sentinel as a plausible date. NOTE:
/// `0xFFFFFFFF` (u32 max) is deliberately NOT listed — it is the genuine HFS+
/// maximum date, not a sentinel. Public so the CLI can flag a sentinel even on
/// the single-format `decode` path (which does not build a [`Candidate`]).
#[must_use]
pub fn sentinel_reason(value: i64) -> Option<&'static str> {
    match value {
        0 => Some("zero / unset"),
        -1 => Some("-1 / all-ones, commonly unset"),
        i64::MAX => Some("0x7FFFFFFFFFFFFFFF, commonly 'never'"),
        _ => None,
    }
}

/// The named plausibility components for one reading (HANDOFF §5b). Each is in
/// `[0, 1]` and emitted verbatim on the `Candidate` so a reviewer can audit the
/// rank instead of trusting an opaque number. NEVER a filter — a low component
/// lowers the rank, it does not hide the reading.
fn score_components(f: &Format, value: i64, instant: PosixNs) -> Vec<(&'static str, f64)> {
    // representable: surfaced only when civil-renderable, so always 1.0 here —
    // emitted explicitly so the component set is complete and self-describing.
    let representable = 1.0;
    let in_window = f64::from(u8::from(
        instant.0 >= f.plausible.0 && instant.0 < f.plausible.1,
    ));
    let granularity = granularity_match(f.strategy, value);
    let magnitude = magnitude_fit(f.strategy, instant);
    let not_sentinel = f64::from(u8::from(sentinel_reason(value).is_none()));
    vec![
        ("representable", representable),
        ("in_window", in_window),
        ("granularity_match", granularity),
        ("magnitude_fit", magnitude),
        ("not_sentinel", not_sentinel),
    ]
}

/// Two years in nanoseconds — the ramp over which an embedded-ID timestamp is
/// considered to have a "realistic" distance from its scheme epoch.
const TWO_YEARS_NS: i128 = 730 * 86_400 * 1_000_000_000;

/// Whether the value's magnitude is consistent with the format's encoding. For
/// linear formats the window already governs magnitude (→ `1.0`). For embedded
/// IDs it is diagnostic: a tiny value decodes to an instant essentially AT the
/// scheme epoch (`id >> shift ≈ 0`), which is implausible for a real ID — so the
/// score ramps from `0.0` at the epoch to `1.0` two years past it.
fn magnitude_fit(strategy: Strategy, instant: PosixNs) -> f64 {
    match strategy {
        Strategy::EmbeddedMillis { epoch_ns, .. } => {
            let past = instant.0 - epoch_ns;
            if past <= 0 {
                0.0
            } else {
                (past as f64 / TWO_YEARS_NS as f64).min(1.0)
            }
        }
        Strategy::LinearInt { .. } | Strategy::LinearFloat { .. } | Strategy::Packed(_) => 1.0,
    }
}

/// How well the raw value's sub-second resolution fits the format's unit. A
/// whole-second value read as nanoseconds is suspiciously coarse (`0.0`); a
/// value carrying real sub-second digits fits perfectly (`1.0`). Coarse units
/// (seconds/days) never penalise. This is the core seconds-vs-ms-vs-µs-vs-ns
/// disambiguation, expressed structurally rather than by "looks human".
fn granularity_match(strategy: Strategy, value: i64) -> f64 {
    let unit: Unit = match strategy {
        Strategy::LinearInt { unit, .. } | Strategy::LinearFloat { unit, .. } => unit,
        Strategy::EmbeddedMillis { .. } => Unit::Millis,
        // Packed civil fields have no linear sub-second unit to mismatch against.
        Strategy::Packed(_) => return 1.0,
    };
    let ssd = unit.sub_second_digits();
    if ssd == 0 {
        return 1.0;
    }
    let tz = trailing_zeros_base10(value).min(ssd);
    1.0 - f64::from(tz) / f64::from(ssd)
}

/// Count of trailing base-10 zeros of `value` (0 for the value `0` itself).
/// Uses `unsigned_abs` so `i64::MIN` cannot panic.
fn trailing_zeros_base10(value: i64) -> u32 {
    let mut n = value.unsigned_abs();
    if n == 0 {
        return 0;
    }
    let mut z = 0;
    while n.is_multiple_of(10) {
        z += 1;
        n /= 10;
    }
    z
}

/// Weighted mean of the named components. `in_window` carries double weight (it
/// is the dominant prior on which readings to surface first); the others weigh
/// one. The result is the overall `[0, 1]` rank.
fn overall_score(components: &[(&'static str, f64)]) -> f64 {
    let weight = |name: &str| match name {
        "in_window" | "magnitude_fit" | "not_sentinel" => 2.0,
        _ => 1.0,
    };
    let (num, den) = components.iter().fold((0.0, 0.0), |(num, den), (n, v)| {
        let w = weight(n);
        (num + w * v, den + w)
    });
    if den == 0.0 {
        0.0
    } else {
        num / den
    }
}

/// Decode hex bytes as little- and big-endian integers of common widths, then
/// run each through [`interpret_int`]. Returns `(byte-decode assumption,
/// candidates)` per width/endianness — the byte layout is itself an assumption.
pub fn interpret_hex(hex: &str) -> Result<Vec<(String, Vec<Candidate>)>, ChronoError> {
    let clean: String = hex
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != ':')
        .collect();
    let clean = clean.strip_prefix("0x").unwrap_or(&clean);
    let bytes = hex::decode(clean).map_err(|_| ChronoError::OutOfRange {
        what: "hex (not valid hex bytes)",
        value: 0,
    })?;
    let mut out = Vec::new();
    for (label, value) in byte_ints(&bytes) {
        out.push((label, interpret_int(value)));
    }
    // Packed formats have an ON-DISK byte order distinct from a linear integer:
    // FAT/DOS stores a date word then a time word, each little-endian. Decode that
    // layout explicitly so an analyst with raw bytes gets the right instant.
    if let Some(four) = bytes.get(..4).and_then(|s| <[u8; 4]>::try_from(s).ok()) {
        let date = u16::from_le_bytes([four[0], four[1]]);
        let time = u16::from_le_bytes([four[2], four[3]]);
        let packed = (i64::from(date) << 16) | i64::from(time);
        if let Some(c) = decode_one("fat", packed) {
            out.push(("FAT/DOS on-disk (date|time LE words)".to_string(), vec![c]));
        }
    }
    Ok(out)
}

/// Decode the first 4 and 8 bytes as LE/BE integers (panic-free, bounds-checked).
fn byte_ints(b: &[u8]) -> Vec<(String, i64)> {
    let mut v = Vec::new();
    if let Some(four) = b.get(..4).and_then(|s| <[u8; 4]>::try_from(s).ok()) {
        v.push(("u32 LE".to_string(), i64::from(u32::from_le_bytes(four))));
        v.push(("u32 BE".to_string(), i64::from(u32::from_be_bytes(four))));
    }
    if let Some(eight) = b.get(..8).and_then(|s| <[u8; 8]>::try_from(s).ok()) {
        if let Ok(n) = i64::try_from(u64::from_le_bytes(eight)) {
            v.push(("u64 LE".to_string(), n));
        }
        if let Ok(n) = i64::try_from(u64::from_be_bytes(eight)) {
            v.push(("u64 BE".to_string(), n));
        }
    }
    v
}

/// Parse a STRING timestamp form: ISO 8601 / RFC 3339, and ASN.1 UTCTime /
/// GeneralizedTime (ITU-T X.680, RFC 5280) as found in X.509 certificates and
/// PKI structures. Returns every form that parses — a string is usually
/// self-describing, so these readings score high. Empty for unparseable input.
#[must_use]
pub fn interpret_string(text: &str) -> Vec<Candidate> {
    let s = text.trim();
    let mut out = Vec::new();
    // RFC 3339 / ISO 8601: jiff parses the offset (or `Z`) and normalises to UTC.
    if let Ok(ts) = s.parse::<jiff::Timestamp>() {
        out.push(string_candidate(
            "iso8601",
            "ISO 8601 / RFC 3339 string",
            "ISO 8601:2019 / RFC 3339",
            PosixNs(ts.as_nanosecond()),
            "parsed as an ISO 8601 / RFC 3339 string (offset normalised to UTC)",
        ));
    }
    if let Some((instant, had_tz)) = parse_asn1_generalizedtime(s) {
        out.push(string_candidate(
            "asn1_generalizedtime",
            "ASN.1 GeneralizedTime",
            "ITU-T X.680 / RFC 5280 §4.1.2.5.2",
            instant,
            &asn1_assumption("GeneralizedTime (4-digit year)", had_tz),
        ));
    }
    if let Some((instant, had_tz)) = parse_asn1_utctime(s) {
        out.push(string_candidate(
            "asn1_utctime",
            "ASN.1 UTCTime",
            "ITU-T X.680 / RFC 5280 §4.1.2.5.1",
            instant,
            &asn1_assumption(
                "UTCTime (2-digit year; RFC 5280 pivot: <50 => 20YY, else 19YY)",
                had_tz,
            ),
        ));
    }
    out
}

/// Build the assumption line for an ASN.1 reading, surfacing the assumed-UTC
/// caveat when the string carried no explicit `Z`/offset (it may be local time).
fn asn1_assumption(kind: &str, had_tz: bool) -> String {
    if had_tz {
        format!("parsed as ASN.1 {kind}")
    } else {
        format!(
            "parsed as ASN.1 {kind}; NO timezone designator — assumed UTC, but may be local time"
        )
    }
}

/// Build a candidate for a self-describing string form. Such inputs are
/// unambiguous once parsed, so they carry a `self_describing` component.
fn string_candidate(
    format_id: &'static str,
    label: &'static str,
    citation: &'static str,
    instant: PosixNs,
    assumption: &str,
) -> Candidate {
    Candidate {
        format_id,
        label,
        citation,
        instant,
        rendered: instant.to_rfc3339(),
        score: 1.0,
        components: vec![
            ("representable", 1.0),
            ("self_describing", 1.0),
            ("not_sentinel", 1.0),
        ],
        assumptions: vec![assumption.to_string()],
        sentinel: false,
    }
}

/// Split a trailing timezone designator (`Z`, `±HHMM`, or none → assume UTC) off
/// a numeric ASN.1 time string, returning the digit core and the offset seconds.
/// Returns `None` when a present offset is malformed or out of range (e.g.
/// `+1260`) — such input must not be silently normalised into a fabricated
/// instant. A `had_tz` flag distinguishes an explicit `Z`/offset from an
/// assumed-UTC fallback so the caller can surface that assumption.
fn split_tz(s: &str) -> Option<(String, i64, bool)> {
    if let Some(core) = s.strip_suffix('Z').or_else(|| s.strip_suffix('z')) {
        return Some((core.to_string(), 0, true));
    }
    if s.len() >= 5 {
        let (core, suf) = s.split_at(s.len() - 5);
        let b = suf.as_bytes();
        if (b[0] == b'+' || b[0] == b'-') && suf[1..].bytes().all(|c| c.is_ascii_digit()) {
            let hh: i64 = suf[1..3].parse().ok()?;
            let mm: i64 = suf[3..5].parse().ok()?;
            if hh > 23 || mm > 59 {
                return None; // out-of-range offset — reject, do not fabricate
            }
            let mag = hh * 3600 + mm * 60;
            return Some((
                core.to_string(),
                if b[0] == b'-' { -mag } else { mag },
                true,
            ));
        }
    }
    Some((s.to_string(), 0, false))
}

/// Build an instant from civil fields at a fixed UTC offset (panic-free).
fn civil_to_posix(
    y: i16,
    mo: i8,
    d: i8,
    h: i8,
    mi: i8,
    s: i8,
    offset_secs: i64,
) -> Option<PosixNs> {
    let dt = jiff::civil::DateTime::new(y, mo, d, h, mi, s, 0).ok()?;
    let off = jiff::tz::Offset::from_seconds(i32::try_from(offset_secs).ok()?).ok()?;
    let zoned = dt.to_zoned(jiff::tz::TimeZone::fixed(off)).ok()?;
    Some(PosixNs(zoned.timestamp().as_nanosecond()))
}

/// `YYYYMMDDHHMMSS` (+ `Z`/offset) — ASN.1 GeneralizedTime.
fn parse_asn1_generalizedtime(s: &str) -> Option<(PosixNs, bool)> {
    let (d, off, had_tz) = split_tz(s)?;
    if d.len() != 14 || !d.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let instant = civil_to_posix(
        d[0..4].parse().ok()?,
        d[4..6].parse().ok()?,
        d[6..8].parse().ok()?,
        d[8..10].parse().ok()?,
        d[10..12].parse().ok()?,
        d[12..14].parse().ok()?,
        off,
    )?;
    Some((instant, had_tz))
}

/// `YYMMDDHHMMSS` (+ `Z`/offset) — ASN.1 UTCTime; 2-digit year per RFC 5280.
fn parse_asn1_utctime(s: &str) -> Option<(PosixNs, bool)> {
    let (d, off, had_tz) = split_tz(s)?;
    if d.len() != 12 || !d.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let yy: i16 = d[0..2].parse().ok()?;
    let year = if yy < 50 { 2000 + yy } else { 1900 + yy };
    let instant = civil_to_posix(
        year,
        d[2..4].parse().ok()?,
        d[4..6].parse().ok()?,
        d[6..8].parse().ok()?,
        d[8..10].parse().ok()?,
        d[10..12].parse().ok()?,
        off,
    )?;
    Some((instant, had_tz))
}
