//! The overlay's runtime font fallback stack must cover every non-ASCII glyph
//! the chrome renders, or those glyphs show as missing-glyph boxes (tofu). This
//! reads the loaded fonts' cmaps and asserts coverage — the regression guard for
//! "a redesign added a symbol no loaded font has" (e.g. ◷ / ⚠, absent from Arial
//! Unicode MS).
//!
//! Two things can make a symbol uncoverable, and only one of them is a defect
//! here. A symbol no LOADED face carries is a real gap — either the chrome grew
//! a glyph nothing covers, or `SYMBOL_FONTS` is missing a face it should list.
//! An ASTRAL symbol on a host with no emoji face installed is neither: nothing
//! shipped can fix it, and failing on it would make this test an audit of the
//! developer's font packages rather than of the code.
//!
//! So the astral symbols are checked only when an emoji face is actually loaded,
//! and when it is not, the skip is REPORTED BY NAME. A silent skip would let the
//! set drift while the test still read green.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph_lens::fonts;
use ttf_parser::Face;

/// Which class of face a symbol needs. A glyph is only CHECKABLE when a face of
/// its class is loaded: asserting a CJK ideograph against a host with no CJK
/// font tests the host's font packages, not this code.
#[derive(PartialEq)]
enum Needs {
    /// Astral-plane (U+10000+): the emoji blocks, carried only by an emoji face.
    Emoji,
    /// Han ideographs — the 干支 pillar labels and era kanji.
    Cjk,
    /// Anything a general symbol or text face carries.
    Any,
}

fn needs(c: char) -> Needs {
    if c.len_utf16() > 1 {
        Needs::Emoji
    } else if matches!(c, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}') {
        Needs::Cjk
    } else {
        Needs::Any
    }
}

#[test]
fn fallback_fonts_cover_every_ui_symbol() {
    let stack = fonts::fallback_fonts();
    if stack.is_empty() {
        eprintln!("no system fallback fonts on this host — skipping coverage check");
        return;
    }
    // Key and face are kept TOGETHER on purpose. Asking "is an emoji face
    // loaded?" of the stack while asking "is this covered?" of the parsed faces
    // lets the two disagree: NotoColorEmoji is a CBDT bitmap font, and a parser
    // that declines it would leave the key present, the glyph uncovered, and the
    // test failing for a symbol nothing could have rendered.
    let faces: Vec<(&str, Face)> = stack
        .iter()
        .filter_map(|(key, bytes)| Face::parse(bytes, 0).ok().map(|f| (*key, f)))
        .collect();
    assert!(
        !faces.is_empty(),
        "loaded fallback bytes did not parse as fonts"
    );
    let covered = |c: char| faces.iter().any(|(_, f)| f.glyph_index(c).is_some());
    let loaded = |k: &str| faces.iter().any(|(key, _)| key.contains(k));

    // (class, is the face for it loaded, what to call it when reporting a skip)
    let classes = [
        (Needs::Any, true, "general"),
        (Needs::Cjk, loaded("cjk"), "CJK"),
        (Needs::Emoji, loaded("emoji"), "emoji"),
    ];

    for (class, face_loaded, label) in classes {
        let syms: Vec<char> = fonts::UI_SYMBOLS
            .iter()
            .copied()
            .filter(|&c| needs(c) == class)
            .collect();
        if syms.is_empty() {
            continue;
        }
        if !face_loaded {
            // Named, never silent: a skip nobody can see is a hole the symbol
            // set drifts into while the test still reads green.
            eprintln!(
                "no {label} face on this host — NOT checked: {syms:?} \
                 (install one to cover them; CI runners and end-user machines differ here)"
            );
            continue;
        }
        let missing: Vec<char> = syms.into_iter().filter(|&c| !covered(c)).collect();
        assert!(
            missing.is_empty(),
            "a {label} face IS loaded but these would render as tofu: {missing:?}"
        );
    }
}
