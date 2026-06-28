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

use crate::{registry::FORMATS, ChronoError, PosixNs, Strategy};

/// One candidate interpretation of a value. Carries its score *components* and
/// *assumptions*, not just a rank — transparency over false confidence.
#[derive(Debug, Clone)]
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
        let Strategy::LinearInt { .. } = f.strategy else {
            continue; // float/packed formats are not numeric-int auto-detected here
        };
        let Ok(instant) = f.decode_int(value) else {
            continue; // overflow → not a valid reading under this format
        };
        let rendered = instant.to_rfc3339();
        if rendered.is_none() {
            continue; // outside civil range → not a usable reading
        }
        let in_window = instant.0 >= f.plausible.0 && instant.0 < f.plausible.1;
        let score = f64::from(u8::from(in_window));
        out.push(Candidate {
            format_id: f.id,
            label: f.label,
            citation: f.citation,
            instant,
            rendered,
            score,
            components: vec![("in_plausible_window", score)],
            assumptions: vec![format!("assumed format: {} [{}]", f.label, f.citation)],
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
