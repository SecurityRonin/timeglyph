//! `scan::word_at` narrows a hovered element's full text to just the token under
//! the cursor. macOS AX reports the cursor as a UTF-16 code-unit offset, so the
//! mapping must account for non-BMP characters before the token.
#![allow(clippy::unwrap_used)]

use timeglyph::scan;

#[test]
fn returns_the_whitespace_delimited_token_at_the_offset() {
    let text = "created=1577836800 modified=1580000000";
    // offset 10 is inside the first token.
    assert_eq!(
        scan::word_at(text, 10).as_deref(),
        Some("created=1577836800")
    );
    // offset 25 is inside the second token.
    assert_eq!(
        scan::word_at(text, 25).as_deref(),
        Some("modified=1580000000")
    );
}

#[test]
fn returns_none_on_whitespace_or_out_of_range() {
    let text = "abc 1577836800";
    assert_eq!(scan::word_at(text, 3), None, "offset 3 is the space");
    assert_eq!(scan::word_at(text, 999), None, "offset past the end");
}

#[test]
fn maps_utf16_offsets_past_non_bmp_characters() {
    // 🕐 (U+1F550) is one char but TWO UTF-16 code units, so the digits start at
    // UTF-16 offset 3 (🕐=2, space=1), not char index 2.
    let text = "🕐 1577836800";
    assert_eq!(scan::word_at(text, 3).as_deref(), Some("1577836800"));
    assert_eq!(scan::word_at(text, 4).as_deref(), Some("1577836800"));
}
