//! System fallback fonts for the overlay.
//!
//! egui's bundled fonts carry no CJK or symbol glyphs, so the overlay loads OS
//! fonts at runtime as fallbacks: a CJK face (the 干支 pillars and lunar date)
//! and a symbol face (the chrome's ◷ / ⚠ marks). Both are appended to every font
//! family; egui falls through to them for any glyph its own fonts lack. Every
//! char in [`UI_SYMBOLS`] the chrome renders MUST be covered by this stack, or it
//! shows as a missing-glyph box (tofu) — `tests/fonts.rs` asserts exactly that
//! against the loaded fonts' cmaps.

/// Non-ASCII glyphs the overlay renders — its own chrome plus the CJK the
/// calendar expansion always shows (the 年月日時 pillar labels and the Japanese
/// era names). Each must have a glyph in some font [`fallback_fonts`] loads, or it
/// renders as tofu. (Arial Unicode MS, the usual CJK face, lacks ◷ U+25F7 and ⚠
/// U+26A0 — hence the symbol face.)
pub const UI_SYMBOLS: &[char] = &[
    '◷', '🌐', '⚙', '☀', '●', '→', '…', '—', '·', '°', // chrome
    '年', '月', '日', '時', // 干支 pillar labels
    '令', '和', '平', '成', '昭', '大', '正', '明', '治', // Japanese era kanji
];

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
    "/System/Library/Fonts/Apple Symbols.ttf",     // macOS
    "/System/Library/Fonts/Apple Color Emoji.ttc", // macOS: 🌐 and friends
    "C:\\Windows\\Fonts\\seguisym.ttf",            // Windows: Segoe UI Symbol
    "C:\\Windows\\Fonts\\seguiemj.ttf",            // Windows: Segoe UI Emoji
    // Linux ships symbol coverage across SEVERAL faces, none of them complete:
    // Symbols2 carries ◷, Symbols carries ⚙, and 🌐 (U+1F310) lives in the emoji
    // face. DejaVu is the near-universal fallback and covers ⚙ where Noto is
    // absent. Every readable one is loaded, not just the first -- see below.
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

/// One stable key per [`SYMBOL_FONTS`] entry, so a loaded face keeps a name in
/// the stack (egui registers fallbacks by key). Kept parallel to that list.
const SYMBOL_KEYS: &[&str] = &[
    "sym-apple",
    "sym-apple-emoji",
    "sym-segoe",
    "sym-segoe-emoji",
    "sym-noto2",
    "sym-noto",
    "sym-noto-emoji",
    "sym-dejavu",
];

/// Per-script faces for scripts a single CJK face doesn't cover. Linux ships one
/// Noto font per script (its CJK face carries no Devanagari/Hebrew/Arabic), so
/// each is loaded separately. macOS's Arial Unicode (via [`CJK_FONTS`]) already
/// covers these, so on macOS/Windows these Linux paths simply aren't found —
/// harmless. Each readable one is appended to the fallback stack under its own key.
pub const SCRIPT_FONTS: &[(&str, &str)] = &[
    (
        "deva",
        "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf",
    ), // बौद्ध Buddhist
    (
        "hebr",
        "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf",
    ), // עברי Hebrew
    (
        "arab",
        "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
    ), // هجري Islamic / خورشیدی Persian
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
    // EVERY readable symbol face, not just the first. Symbol coverage is spread
    // across faces that do not subsume one another -- Noto Symbols2 has ◷ but not
    // ⚙, Noto Symbols has ⚙ but not 🌐, and 🌐 needs the emoji face. Taking only
    // the first meant a host with Symbols2 installed rendered ⚙ and 🌐 as tofu
    // while a perfectly good face sat unread beside it. Found by tests/fonts.rs
    // on a Linux host that has exactly that combination.
    for (i, path) in SYMBOL_FONTS.iter().enumerate() {
        if let Ok(bytes) = std::fs::read(path) {
            stack.push((SYMBOL_KEYS[i], bytes));
        }
    }
    // Per-script faces (Linux ships these separately; macOS/Windows cover them via
    // the CJK/pan-Unicode face above, so these paths simply won't exist there).
    for (key, path) in SCRIPT_FONTS {
        if let Ok(bytes) = std::fs::read(path) {
            stack.push((key, bytes));
        }
    }
    stack
}
