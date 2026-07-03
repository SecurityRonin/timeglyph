//! Pure scan core: pull numeric runs out of arbitrary UI text and turn each
//! into timeglyph's top ranked datetime readings, rendered in a chosen zone. No
//! platform or GUI dependency — this is the testable half of the inspector.

use std::fmt;

use timeglyph::{interpret, PosixNs, RenderZone, TzSemantics};

/// One decoded reading of a number: which format, the rendered instant, and the
/// human label — kept as separate fields so the GUI can style each distinctly.
/// `Display` renders the console form `"<format>  <rendered>  (<label>)"`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// The format identifier (e.g. `unix`, `webkit`).
    pub format_id: String,
    /// The rendered datetime, expressed in the requested display zone.
    pub rendered: String,
    /// The human-readable format label (e.g. `Unix time (seconds)`).
    pub label: String,
    /// True when the value is naive *local* wall-clock (no UTC anchor): the
    /// display zone is NOT applied, and the reading carries no zone designator.
    pub local: bool,
    /// The absolute instant, kept so a caller can re-express it (e.g. the 干支
    /// expansion) without re-decoding.
    pub instant: PosixNs,
    /// The engine's overall plausibility score in `[0, 1]` — the ranking signal,
    /// shown as a confidence percentage (see [`confidence_pct`]). A heuristic
    /// plausibility measure, not a calibrated probability.
    pub score: f64,
    /// The named component scores behind [`score`](Self::score), for a breakdown
    /// tooltip (e.g. `("in_window", 1.0)`).
    pub components: Vec<(&'static str, f64)>,
}

/// Render a `[0, 1]` plausibility [`score`](Reading::score) as a whole-number
/// percentage, clamped to `0..=100`.
#[must_use]
pub fn confidence_pct(score: f64) -> u8 {
    (score.clamp(0.0, 1.0) * 100.0).round() as u8
}

impl fmt::Display for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<12} {}  ({})",
            self.format_id, self.rendered, self.label
        )
    }
}

/// One number found in the inspected text, with its top datetime readings.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberReadings {
    /// The numeric run as it appeared in the text.
    pub number: String,
    /// The top ranked readings.
    pub readings: Vec<Reading>,
}

/// Minimum digit count for a run to be treated as a possible timestamp. Shorter
/// runs (counts, ids, 4-digit years) are dropped so the overlay stays quiet.
const MIN_DIGITS: usize = 8;

/// Extract candidate numeric runs (>= [`MIN_DIGITS`] consecutive ASCII digits)
/// from `text`, in order of appearance.
#[must_use]
pub fn scan_numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if cur.len() >= MIN_DIGITS {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// Render `instant` for display in `zone`, honoring the format's tz semantics.
///
/// Only a [`TzSemantics::Utc`] value has a UTC anchor, so only it may be shifted
/// into the display zone (with an explicit offset). A [`TzSemantics::LocalNaive`]
/// value is wall-clock with no offset — shifting it would fabricate meaning, so
/// it is shown as-is (the misleading `Z` stripped) and reported `local`. An
/// [`TzSemantics::OffsetEmbedded`] value already carries its own offset, so the
/// display zone is not applied either. Returns `(rendered, is_local)`.
///
/// `native` is the format's own (UTC) rendering, used as the fallback and the
/// source for the wall-clock / offset-embedded cases.
#[must_use]
pub fn render_in_zone(
    tz: TzSemantics,
    instant: PosixNs,
    native: &str,
    zone: &RenderZone,
) -> (String, bool) {
    match tz {
        TzSemantics::Utc => (
            instant.render(zone).unwrap_or_else(|| native.to_string()),
            false,
        ),
        TzSemantics::LocalNaive => (native.trim_end_matches('Z').to_string(), true),
        TzSemantics::OffsetEmbedded => (native.to_string(), false),
    }
}

/// The top `max` *in-window* datetime readings for one numeric string, each
/// rendered in `zone` (semantics-aware — see [`render_in_zone`]). Empty when the
/// number does not parse or has no confident (in-window, non-sentinel) reading.
#[must_use]
pub fn readings_for(number: &str, max: usize, zone: &RenderZone) -> Vec<Reading> {
    let Ok(value) = number.parse::<i64>() else {
        return Vec::new();
    };
    interpret::interpret_int(value)
        .into_iter()
        .filter(|c| {
            !c.sentinel
                && c.rendered.is_some()
                && c.components
                    .iter()
                    .any(|(n, v)| *n == "in_window" && *v > 0.0)
        })
        .take(max)
        .map(|c| reading_from(c, zone))
        .collect()
}

/// Build one [`Reading`] from a candidate, rendered in `zone` (semantics-aware).
fn reading_from(c: interpret::Candidate, zone: &RenderZone) -> Reading {
    let tz = timeglyph::format(c.format_id)
        .map(|f| f.tz)
        .unwrap_or(TzSemantics::Utc);
    let native = c.rendered.clone().unwrap_or_default();
    let (rendered, local) = render_in_zone(tz, c.instant, &native, zone);
    Reading {
        format_id: c.format_id.to_string(),
        rendered,
        label: c.label.to_string(),
        local,
        instant: c.instant,
        score: c.score,
        components: c.components,
    }
}

/// Confident readings for a candidate datetime *string* — the self-describing
/// forms (ISO-8601/RFC-3339, RFC-2822, HTTP-date, EXIF, ASN.1, …), rendered in
/// `zone`. Empty when `text` is not a datetime string. Unlike [`readings_for`]
/// there is no in-window gate: a parsed string form is evidence in itself.
#[must_use]
pub fn readings_for_string(text: &str, zone: &RenderZone) -> Vec<Reading> {
    interpret::interpret_string(text)
        .into_iter()
        .filter(|c| !c.sentinel && c.rendered.is_some())
        .map(|c| reading_from(c, zone))
        .collect()
}

/// Candidate datetime-string substrings of `text`: the whole text, each line,
/// and each whitespace token (deduped, >= 8 chars). Pure numeric runs are
/// excluded — those are the integer path's job ([`scan_numbers`]).
fn datetime_candidates(text: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    let mut push = |s: &str| {
        let t = s.trim();
        if t.len() >= 8 && !t.bytes().all(|b| b.is_ascii_digit()) && seen.insert(t.to_string()) {
            out.push(t.to_string());
        }
    };
    push(text);
    text.lines().for_each(&mut push);
    text.split_whitespace().for_each(&mut push);
    out
}

/// Inspect a block of UI text: every long number AND every rendered datetime
/// string paired with its top readings, rendered in `zone`. Items with no
/// confident reading are dropped, so noise stays off the screen.
#[must_use]
pub fn inspect_text(text: &str, max_per_number: usize, zone: &RenderZone) -> Vec<NumberReadings> {
    let mut out: Vec<NumberReadings> = scan_numbers(text)
        .into_iter()
        .filter_map(|number| {
            let readings = readings_for(&number, max_per_number, zone);
            (!readings.is_empty()).then_some(NumberReadings { number, readings })
        })
        .collect();
    for cand in datetime_candidates(text) {
        let readings: Vec<Reading> = readings_for_string(&cand, zone)
            .into_iter()
            .take(max_per_number)
            .collect();
        if !readings.is_empty() {
            out.push(NumberReadings {
                number: cand,
                readings,
            });
        }
    }
    out
}
