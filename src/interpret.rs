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

use crate::{registry::FORMATS, ChronoError, Format, LeapSemantics, PosixNs, Strategy, Unit};

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
}

/// Interpret a raw integer across every `LinearInt` format. Returns ALL readings
/// that render to a civil date, ranked by score (descending), then by id for
/// determinism. The caller MUST present these as candidates, not a single answer.
#[must_use]
pub fn interpret_int(value: i64) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for f in FORMATS {
        // Any integer-decodable strategy is a candidate; float-only strategies
        // return Err here and are skipped (they are decoded via --from).
        let Ok(instant) = f.decode_int(value) else {
            continue;
        };
        let Some(rendered) = instant.to_rfc3339() else {
            continue; // outside civil range → not a usable reading
        };
        let components = score_components(f, value, instant);
        let score = overall_score(&components);
        out.push(Candidate {
            format_id: f.id,
            label: f.label,
            citation: f.citation,
            instant,
            rendered: Some(rendered),
            score,
            components,
            assumptions: assumptions(f),
        });
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.format_id.cmp(b.format_id))
    });
    out
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
    out
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
    vec![
        ("representable", representable),
        ("in_window", in_window),
        ("granularity_match", granularity),
        ("magnitude_fit", magnitude),
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
        Strategy::LinearInt { .. } | Strategy::LinearFloat { .. } => 1.0,
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
        "in_window" | "magnitude_fit" => 2.0,
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
