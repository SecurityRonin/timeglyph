//! Tests for char-safe text truncation (the header caption must never panic on
//! multi-byte input — egui 0.29 Label::truncate byte-slices and crashes).
#![allow(clippy::unwrap_used)]

use timeglyph_spy::text::ellipsize;

#[test]
fn ellipsize_never_slices_mid_char() {
    // Multi-byte content (·, 干, 🕰) truncated at any length must stay valid and
    // bounded — never a mid-char byte slice (the crash we are fixing).
    let s = "a·干b🕰c ".repeat(40);
    for max in [1usize, 2, 3, 7, 20, 71, 72, 73] {
        let e = ellipsize(&s, max);
        assert!(e.chars().count() <= max, "max={max}: {e:?}");
    }
}

#[test]
fn short_text_passes_through_and_long_gets_ellipsis() {
    assert_eq!(ellipsize("hello", 20), "hello");
    let e = ellipsize(&"x".repeat(100), 10);
    assert_eq!(e.chars().count(), 10);
    assert!(e.ends_with('…'));
}
