//! Scan arbitrary text for timestamp candidates and decode each into ranked
//! readings. Three extractors — long digit runs ([`scan_numbers`]),
//! self-describing datetime strings ([`datetime_candidates`]), and raw-hex tokens
//! ([`hex_candidates`]) — feed [`interpret`](crate::interpret); [`inspect_text`]
//! ties them together. Pure and GUI-free: it powers both the CLI `scan` command
//! and the timeglyph-lens overlay.

use std::fmt;

use crate::{
    datefmt::{format_instant, format_naive},
    interpret, DateStyle, PosixNs, RenderZone, TzSemantics,
};

/// One decoded reading of a value: which format, the rendered instant, and the
/// human label — kept as separate fields so a caller can style each distinctly.
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

/// The weekday name of a reading's *displayed* value — parsed from its leading
/// ISO date (`YYYY-MM-DD`), so it always matches the shown date regardless of
/// zone or format. `None` if the value doesn't start with a valid date.
#[must_use]
pub fn weekday(rendered: &str) -> Option<&'static str> {
    let date: jiff::civil::Date = rendered.get(..10)?.parse().ok()?;
    Some(match date.weekday() {
        jiff::civil::Weekday::Monday => "Monday",
        jiff::civil::Weekday::Tuesday => "Tuesday",
        jiff::civil::Weekday::Wednesday => "Wednesday",
        jiff::civil::Weekday::Thursday => "Thursday",
        jiff::civil::Weekday::Friday => "Friday",
        jiff::civil::Weekday::Saturday => "Saturday",
        jiff::civil::Weekday::Sunday => "Sunday",
    })
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

/// One value found in the scanned text, with its top datetime readings.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberReadings {
    /// The numeric run (or datetime string) as it appeared in the text.
    pub number: String,
    /// The top ranked readings.
    pub readings: Vec<Reading>,
}

/// Default minimum digit count for a run to be treated as a possible timestamp.
/// Shorter runs (counts, ids, 4-digit years) are dropped so scans stay quiet.
pub const MIN_DIGITS: usize = 8;

/// The whitespace-delimited token of `text` containing the UTF-16 code-unit
/// offset `utf16_offset` — how macOS Accessibility reports the character under a
/// screen point. Used to narrow a hovered element's *entire* value (e.g. an
/// iTerm buffer) to just the token under the cursor before decoding.
///
/// `None` if the offset is out of range or lands on whitespace (nothing to
/// decode there). The offset is in UTF-16 units, so it is mapped past non-BMP
/// characters to the right Unicode scalar.
#[must_use]
pub fn word_at(text: &str, utf16_offset: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    // Find the char whose UTF-16 span contains the offset.
    let mut acc = 0usize;
    let mut idx = None;
    for (i, ch) in chars.iter().enumerate() {
        let next = acc + ch.len_utf16();
        if utf16_offset < next {
            idx = Some(i);
            break;
        }
        acc = next;
    }
    let idx = idx?;
    if chars[idx].is_whitespace() {
        return None;
    }
    let mut start = idx;
    while start > 0 && !chars[start - 1].is_whitespace() {
        start -= 1;
    }
    let mut end = idx;
    while end + 1 < chars.len() && !chars[end + 1].is_whitespace() {
        end += 1;
    }
    Some(chars[start..=end].iter().collect())
}

/// Extract candidate numeric runs (>= `min_digits` consecutive ASCII digits) from
/// `text`, in order of appearance.
#[must_use]
pub fn scan_numbers_min(text: &str, min_digits: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut digits = 0usize;
    let mut has_dot = false;
    let flush =
        |cur: &mut String, digits: &mut usize, has_dot: &mut bool, out: &mut Vec<String>| {
            // The '.' is only ever pushed between digits, so a token never ends
            // on one — the digit count (not byte length) is the run gate.
            if *digits >= min_digits {
                out.push(std::mem::take(cur));
            } else {
                cur.clear();
            }
            *digits = 0;
            *has_dot = false;
        };
    let chars: Vec<char> = text.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_digit() {
            cur.push(ch);
            digits += 1;
        } else if ch == '.'
            && !has_dot
            && !cur.is_empty()
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
        {
            // A single decimal point between digits is part of a float literal
            // (e.g. CFAbsoluteTime 606940977.71577), not a token boundary.
            cur.push('.');
            has_dot = true;
        } else {
            flush(&mut cur, &mut digits, &mut has_dot, &mut out);
        }
    }
    flush(&mut cur, &mut digits, &mut has_dot, &mut out);
    out
}

/// [`scan_numbers_min`] with the default [`MIN_DIGITS`] threshold.
#[must_use]
pub fn scan_numbers(text: &str) -> Vec<String> {
    scan_numbers_min(text, MIN_DIGITS)
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
/// source for the wall-clock / offset-embedded cases. `style` shapes the
/// displayed text for the shiftable ([`TzSemantics::Utc`]) case only; the
/// wall-clock and offset-embedded cases keep their own native rendering.
#[must_use]
pub fn render_in_zone(
    tz: TzSemantics,
    instant: PosixNs,
    native: &str,
    zone: &RenderZone,
    style: DateStyle,
) -> (String, bool) {
    match tz {
        // Out of civil range: keep the format's own native rendering rather than
        // a style-formatted placeholder (same fallback the plain render had).
        TzSemantics::Utc if instant.render(zone).is_none() => (native.to_string(), false),
        TzSemantics::Utc => (format_instant(instant, zone, style), false),
        TzSemantics::LocalNaive => (format_naive(instant, style), true),
        TzSemantics::OffsetEmbedded => (native.to_string(), false),
    }
}

/// The confidence gate shared by the numeric and hex scan paths, so they cannot
/// drift: a reading surfaces in a normal scan only if it renders, is not a
/// sentinel, and is in-window. `include_all` is the exhaustive escape — it keeps
/// every rendered reading (sentinels + out-of-window included).
fn confident(c: &interpret::Candidate, include_all: bool) -> bool {
    c.rendered.is_some()
        && (include_all
            || (!c.sentinel
                && c.components
                    .iter()
                    .any(|(n, v)| *n == "in_window" && *v > 0.0)))
}

/// The top `max` *in-window* datetime readings for one numeric string, each
/// rendered in `zone` (semantics-aware — see [`render_in_zone`]). Empty when the
/// number does not parse or has no confident (in-window, non-sentinel) reading.
#[must_use]
pub fn readings_for(number: &str, max: usize, zone: &RenderZone) -> Vec<Reading> {
    readings_for_opts(number, max, false, zone, DateStyle::Iso8601)
}

/// [`readings_for`] with an `include_all` escape: when true, keeps sentinel and
/// out-of-window candidates too (for an exhaustive scan). `style` shapes the
/// displayed datetime text.
#[must_use]
pub fn readings_for_opts(
    number: &str,
    max: usize,
    include_all: bool,
    zone: &RenderZone,
    style: DateStyle,
) -> Vec<Reading> {
    // Integer epochs first; a fractional literal (CFAbsoluteTime, OLE date, …)
    // routes to the float-strategy decoders instead, preserving its sub-seconds.
    let candidates = if let Ok(value) = number.parse::<i64>() {
        interpret::interpret_int(value)
    } else if let Ok(value) = number.parse::<f64>() {
        interpret::interpret_float(value)
    } else {
        return Vec::new();
    };
    candidates
        .into_iter()
        .filter(|c| confident(c, include_all))
        .take(max)
        .map(|c| reading_from(c, zone, style))
        .collect()
}

/// Build one [`Reading`] from a candidate, rendered in `zone` with `style`
/// (semantics-aware).
fn reading_from(c: interpret::Candidate, zone: &RenderZone, style: DateStyle) -> Reading {
    let tz = crate::format(c.format_id).map_or(TzSemantics::Utc, |f| f.tz);
    let native = c.rendered.clone().unwrap_or_default();
    let (rendered, local) = render_in_zone(tz, c.instant, &native, zone, style);
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
    string_readings_opts(text, false, zone, DateStyle::Iso8601)
}

fn string_readings_opts(
    text: &str,
    include_all: bool,
    zone: &RenderZone,
    style: DateStyle,
) -> Vec<Reading> {
    interpret::interpret_string(text)
        .into_iter()
        .filter(|c| c.rendered.is_some() && (include_all || !c.sentinel))
        .map(|c| reading_from(c, zone, style))
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

/// Candidate raw-hex tokens in `text`: whitespace-delimited tokens that are
/// either `0x`/`0X`-prefixed hex (any length) OR a bare hex run that carries at
/// least one `a-f`/`A-F` letter AND is >= 8 hex chars (>= 4 bytes) and even
/// length. The letter + length floor is deliberate: without it every short
/// decimal run or lowercase word (`cafe`, `dead`) would be decoded as bytes and
/// flood the scan with noise. A `0x` prefix is an explicit intent signal, so it
/// bypasses the letter/length floor. Deduped, in order of appearance.
fn hex_candidates(text: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for tok in text.split_whitespace() {
        let is_hex = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit());
        let accept = if let Some(rest) = tok.strip_prefix("0x").or_else(|| tok.strip_prefix("0X")) {
            is_hex(rest)
        } else {
            tok.len() >= 8
                && tok.len().is_multiple_of(2)
                && is_hex(tok)
                && tok.bytes().any(|b| b.is_ascii_alphabetic())
        };
        if accept && seen.insert(tok.to_string()) {
            out.push(tok.to_string());
        }
    }
    out
}

/// The top `max` readings for one raw-hex token, folding every byte-layout
/// group's candidates from [`interpret::interpret_hex`] into flat [`Reading`]s
/// (semantics-aware, rendered in `zone`). `include_all` keeps sentinel and
/// out-of-window readings. Empty when the token is not valid hex or yields none.
fn hex_readings_opts(
    token: &str,
    max: usize,
    include_all: bool,
    zone: &RenderZone,
    style: DateStyle,
) -> Vec<Reading> {
    let Ok(groups) = interpret::interpret_hex(token) else {
        return Vec::new();
    };
    groups
        .into_iter()
        .flat_map(|(_layout, cands)| cands)
        .filter(|c| confident(c, include_all))
        .take(max)
        .map(|c| reading_from(c, zone, style))
        .collect()
}

/// Scan a block of text with a custom minimum-digit threshold: every numeric run
/// AND every datetime string paired with its top readings, rendered in `zone`.
/// Items with no confident reading are dropped, so noise stays out.
#[must_use]
pub fn inspect_text_min(
    text: &str,
    max_per_number: usize,
    min_digits: usize,
    zone: &RenderZone,
) -> Vec<NumberReadings> {
    inspect_text_opts(
        text,
        max_per_number,
        min_digits,
        false,
        zone,
        DateStyle::Iso8601,
    )
}

/// [`inspect_text_min`] with an `include_all` escape: keep sentinel and
/// out-of-window readings too (an exhaustive, noisier scan). `style` shapes the
/// displayed datetime text of each reading.
#[must_use]
pub fn inspect_text_opts(
    text: &str,
    max_per_number: usize,
    min_digits: usize,
    include_all: bool,
    zone: &RenderZone,
    style: DateStyle,
) -> Vec<NumberReadings> {
    let mut out: Vec<NumberReadings> = scan_numbers_min(text, min_digits)
        .into_iter()
        .filter_map(|number| {
            let readings = readings_for_opts(&number, max_per_number, include_all, zone, style);
            (!readings.is_empty()).then_some(NumberReadings { number, readings })
        })
        .collect();
    for cand in datetime_candidates(text) {
        let readings: Vec<Reading> = string_readings_opts(&cand, include_all, zone, style)
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
    // Raw-hex tokens (0x-prefixed, or hex-with-letters above the byte floor) — so
    // `scan` and the lens decode on-disk byte layouts too. Skip any token already
    // emitted by the numeric/datetime passes so a value is not double-counted.
    let already: std::collections::BTreeSet<&str> =
        out.iter().map(|nr| nr.number.as_str()).collect();
    let hex: Vec<NumberReadings> = hex_candidates(text)
        .into_iter()
        .filter(|tok| !already.contains(tok.as_str()))
        .filter_map(|token| {
            let readings = hex_readings_opts(&token, max_per_number, include_all, zone, style);
            (!readings.is_empty()).then_some(NumberReadings {
                number: token,
                readings,
            })
        })
        .collect();
    out.extend(hex);
    out
}

/// [`inspect_text_min`] with the default [`MIN_DIGITS`] threshold.
#[must_use]
pub fn inspect_text(text: &str, max_per_number: usize, zone: &RenderZone) -> Vec<NumberReadings> {
    inspect_text_min(text, max_per_number, MIN_DIGITS, zone)
}
