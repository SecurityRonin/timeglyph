//! Char-safe text helpers for the overlay. egui 0.29's `Label::truncate()`
//! byte-slices the galley and panics on multi-byte text, so the overlay
//! truncates captions itself, by character.

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
