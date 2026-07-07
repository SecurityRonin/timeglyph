//! Char-safe text helpers for the overlay. egui 0.29's `Label::truncate()`
//! byte-slices the galley and panics on multi-byte text, so the overlay
//! truncates captions itself, by character.

use timeglyph::scan::Reading;
use timeglyph::{DateStyle, RenderZone};

/// The string a click on a reading row copies to the clipboard.
///
/// Mirrors the `shown` computation in `datetime_cell`: local-naive readings
/// return `r.rendered` unchanged (no UTC anchor to shift); zone-shiftable
/// readings are re-formatted for the active `zone` and `style`.
#[must_use]
pub fn copy_text_for(r: &Reading, zone: &RenderZone, style: DateStyle) -> String {
    if r.local {
        r.rendered.clone()
    } else {
        timeglyph::datefmt::format_instant(r.instant, zone, style)
    }
}

/// Truncate `s` to at most `max` characters, appending `…` when shortened.
/// Char-safe: never slices inside a multi-byte character (unlike a byte slice).
#[must_use]
pub fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}
