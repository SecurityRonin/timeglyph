//! The overlay's runtime font fallback stack must cover every non-ASCII glyph
//! the chrome renders, or those glyphs show as missing-glyph boxes (tofu). This
//! reads the loaded fonts' cmaps and asserts coverage — the regression guard for
//! "a redesign added a symbol no loaded font has" (e.g. ◷ / ⚠, absent from Arial
//! Unicode MS). Host-font-gated: skips cleanly when no system fallback is found.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph_lens::fonts;
use ttf_parser::Face;

#[test]
fn fallback_fonts_cover_every_ui_symbol() {
    let stack = fonts::fallback_fonts();
    if stack.is_empty() {
        eprintln!("no system fallback fonts on this host — skipping coverage check");
        return;
    }
    let faces: Vec<Face> = stack
        .iter()
        .filter_map(|(_, bytes)| Face::parse(bytes, 0).ok())
        .collect();
    assert!(
        !faces.is_empty(),
        "loaded fallback bytes did not parse as fonts"
    );

    let uncovered: Vec<char> = fonts::UI_SYMBOLS
        .iter()
        .copied()
        .filter(|&c| !faces.iter().any(|f| f.glyph_index(c).is_some()))
        .collect();

    assert!(
        uncovered.is_empty(),
        "UI symbols with no glyph in any loaded fallback font (would render as tofu): {uncovered:?}"
    );
}
