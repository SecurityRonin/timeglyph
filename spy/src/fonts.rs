//! System fallback fonts for the overlay.
//!
//! egui's bundled fonts carry no CJK or symbol glyphs, so the overlay loads OS
//! fonts at runtime as fallbacks: a CJK face (the 干支 pillars and lunar date)
//! and a symbol face (the chrome's ◷ / ⚠ marks). Both are appended to every font
//! family; egui falls through to them for any glyph its own fonts lack. Every
//! char in [`UI_SYMBOLS`] the chrome renders MUST be covered by this stack, or it
//! shows as a missing-glyph box (tofu) — `tests/fonts.rs` asserts exactly that
//! against the loaded fonts' cmaps.

/// Non-ASCII glyphs the overlay's own chrome renders. Each must have a glyph in
/// some font [`fallback_fonts`] loads, or it renders as tofu. (Arial Unicode MS,
/// the usual CJK face, lacks ◷ U+25F7 and ⚠ U+26A0 — hence the symbol face.)
pub const UI_SYMBOLS: &[char] = &['◷', '⚠', '🌐', '⚙', '→', '…', '—', '·', '°'];

/// CJK-capable faces, most-preferred first. A `.ttc` collection loads face 0.
pub const CJK_FONTS: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf", // macOS: CJK + most symbols
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/System/Library/Fonts/PingFang.ttc",
    "C:\\Windows\\Fonts\\msyh.ttc",   // Windows: Microsoft YaHei
    "C:\\Windows\\Fonts\\simsun.ttc", // SimSun
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", // Linux
];

/// Symbol faces covering the geometric / warning marks the CJK faces lack
/// (◷ U+25F7, ⚠ U+26A0).
pub const SYMBOL_FONTS: &[&str] = &[
    "/System/Library/Fonts/Apple Symbols.ttf", // macOS
    "C:\\Windows\\Fonts\\seguisym.ttf",        // Windows: Segoe UI Symbol
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf", // Linux
];

/// The first path that reads, as owned bytes.
fn first_readable(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

/// The ordered `(family-key, bytes)` fallback stack to append to egui's font
/// families: the CJK face then the symbol face (whichever are present on the
/// host). Empty when the host has neither — the overlay still runs, degrading
/// missing glyphs to tofu rather than failing.
pub fn fallback_fonts() -> Vec<(&'static str, Vec<u8>)> {
    let mut stack = Vec::new();
    if let Some(bytes) = first_readable(CJK_FONTS) {
        stack.push(("cjk", bytes));
    }
    if let Some(bytes) = first_readable(SYMBOL_FONTS) {
        stack.push(("sym", bytes));
    }
    stack
}
