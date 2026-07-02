//! Pure scan core: pull numeric runs out of arbitrary UI text and turn each
//! into timeglyph's top ranked datetime readings. No platform or GUI dependency
//! — this is the testable half of the inspector.

use std::fmt;

use timeglyph::interpret;

/// One decoded reading of a number: which format, the rendered instant, and the
/// human label — kept as separate fields so the GUI can style each distinctly.
/// `Display` renders the console form `"<format>  <rendered>  (<label>)"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    /// The format identifier (e.g. `unix`, `webkit`).
    pub format_id: String,
    /// The rendered datetime (RFC 3339).
    pub rendered: String,
    /// The human-readable format label (e.g. `Unix time (seconds)`).
    pub label: String,
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// The top `max` *in-window* datetime readings for one numeric string. Empty when
/// the number does not parse or has no confident (in-window, non-sentinel) reading.
#[must_use]
pub fn readings_for(number: &str, max: usize) -> Vec<Reading> {
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
        .map(|c| Reading {
            format_id: c.format_id.to_string(),
            rendered: c.rendered.unwrap_or_default(),
            label: c.label.to_string(),
        })
        .collect()
}

/// Inspect a block of UI text: every long number paired with its top readings.
/// Numbers with no confident reading are dropped, so noise stays off the screen.
#[must_use]
pub fn inspect_text(text: &str, max_per_number: usize) -> Vec<NumberReadings> {
    scan_numbers(text)
        .into_iter()
        .filter_map(|number| {
            let readings = readings_for(&number, max_per_number);
            (!readings.is_empty()).then_some(NumberReadings { number, readings })
        })
        .collect()
}
