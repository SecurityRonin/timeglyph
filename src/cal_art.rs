//! ASCII art for the `cal` visual layer: moon-phase discs and seasonal scene
//! tiles. These are a *rendering* of values computed in [`crate::cal`] (the
//! phase index, the season) — never a value themselves — so the art is a pure
//! lookup keyed by an integer, snapshot-tested under the tier-1 astronomy tests.
//! `@` = lit, `.` = dark; single-width ASCII only (no box-drawing).
#![cfg(feature = "lunisolar")]

/// A single-width glyph per phase index (0 = new .. 4 = full .. 7 = waning
/// crescent), for compact views where a 7-line disc does not fit.
pub const PHASE_GLYPH: [char; 8] = ['N', ')', ')', 'O', 'F', 'O', '(', '('];

/// Seven-line shaded moon discs, indexed by phase (0 = new .. 7). `@` lit, `.`
/// dark; the terminator sweeps left→right across the waxing half and back.
const MOON_ART: [[&str; 7]; 8] = [
    // 0 New
    [
        "    . . . . .",
        "  . . . . . . .",
        " . . . . . . . .",
        " . . . . . . . .",
        " . . . . . . . .",
        "  . . . . . . .",
        "    . . . . .",
    ],
    // 1 Waxing crescent (thin lit right)
    [
        "    . . . . @",
        "  . . . . . @ @",
        " . . . . . . @ @",
        " . . . . . . @ @",
        " . . . . . . @ @",
        "  . . . . . @ @",
        "    . . . . @",
    ],
    // 2 First quarter (lit right half)
    [
        "    . . @ @ @",
        "  . . . @ @ @ @",
        " . . . . @ @ @ @",
        " . . . . @ @ @ @",
        " . . . . @ @ @ @",
        "  . . . @ @ @ @",
        "    . . @ @ @",
    ],
    // 3 Waxing gibbous
    [
        "    . @ @ @ @",
        "  . . @ @ @ @ @",
        " . . @ @ @ @ @ @",
        " . . @ @ @ @ @ @",
        " . . @ @ @ @ @ @",
        "  . . @ @ @ @ @",
        "    . @ @ @ @",
    ],
    // 4 Full
    [
        "    @ @ @ @ @",
        "  @ @ @ @ @ @ @",
        " @ @ @ @ @ @ @ @",
        " @ @ @ @ @ @ @ @",
        " @ @ @ @ @ @ @ @",
        "  @ @ @ @ @ @ @",
        "    @ @ @ @ @",
    ],
    // 5 Waning gibbous
    [
        "    @ @ @ @ .",
        "  @ @ @ @ @ . .",
        " @ @ @ @ @ @ . .",
        " @ @ @ @ @ @ . .",
        " @ @ @ @ @ @ . .",
        "  @ @ @ @ @ . .",
        "    @ @ @ @ .",
    ],
    // 6 Last quarter (lit left half)
    [
        "    @ @ . . .",
        "  @ @ @ @ . . .",
        " @ @ @ @ . . . .",
        " @ @ @ @ . . . .",
        " @ @ @ @ . . . .",
        "  @ @ @ @ . . .",
        "    @ @ . . .",
    ],
    // 7 Waning crescent (thin lit left)
    [
        "    @ . . . .",
        "  @ @ . . . . .",
        " @ @ . . . . . .",
        " @ @ . . . . . .",
        " @ @ . . . . . .",
        "  @ @ . . . . .",
        "    @ . . . .",
    ],
];

/// The seven-line moon disc for a phase index (`0..=7`, clamped).
#[must_use]
pub fn moon_art(phase_index: u8) -> &'static [&'static str; 7] {
    &MOON_ART[(phase_index as usize) % 8]
}

/// Four seasonal scene tiles, indexed 0=spring 1=summer 2=autumn 3=winter.
const SEASON_TILE: [[&str; 5]; 4] = [
    // spring: blossom
    [
        "   *  .  *",
        "  .  \\|/  .",
        " *---- o ----*",
        "  .  /|\\  .",
        "   *  '  *",
    ],
    // summer: beach (sun + umbrella + waves)
    [
        "  \\ | /     ___",
        " -- O --   /___\\",
        "  / | \\      |",
        " ~~~~~~~~~~~~|~~~",
        "     beach",
    ],
    // autumn: falling leaves
    [
        "   ,  &   ,",
        "  &   ,  &  ,",
        " ,  &   ,   &",
        "   &   ,  &",
        "  ~~~~~~~~~~~",
    ],
    // winter: snowman
    [
        "     _===_",
        "    (.o.o.)",
        "    ( >^< )",
        "   (( : : ))",
        "  *  *  *  *  *",
    ],
];

/// The five-line scene tile for a season index (`0..=3`, clamped).
#[must_use]
pub fn season_tile(season_index: u8) -> &'static [&'static str; 5] {
    &SEASON_TILE[(season_index as usize) % 4]
}
