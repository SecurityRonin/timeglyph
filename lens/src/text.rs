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

/// The ISO-anchored datetime the weekday / public-holiday labels must be derived from:
/// the reading's instant re-expressed in the **current** display zone.
///
/// The datetime cell renders from [`Reading::instant`] for the live zone, so deriving
/// the labels from the reading's baked `rendered` string made them describe whatever
/// zone was active when the reading was decoded — after a zone change across a date
/// boundary the card showed one date and labelled it with another day's weekday and
/// holidays.
///
/// Two properties this preserves:
/// - A **local-naive** reading carries no offset, so shifting it would fabricate
///   meaning; it keeps its own wall-clock rendering whatever the display zone is.
/// - The result stays **ISO-anchored regardless of `DateStyle`**, because
///   [`scan::weekday`] and the holiday lookup parse `YYYY-MM-DD` from the front. Using
///   the styled display text here would make the labels vanish under a non-ISO style
///   rather than be correct.
///
/// `None` only when the instant is outside the civil range, the same contract as
/// [`timeglyph::PosixNs::render`].
#[must_use]
pub fn label_basis(r: &Reading, zone: &RenderZone, _style: DateStyle) -> Option<String> {
    if r.local {
        // Never shifted — see above.
        Some(r.rendered.clone())
    } else {
        r.instant.render(zone)
    }
}
