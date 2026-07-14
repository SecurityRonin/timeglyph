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
const SEASON_TILE: [[&str; 6]; 4] = [
    // spring: blossom
    [
        "   *  .  *",
        "  .  \\|/  .",
        " *---- o ----*",
        "  .  /|\\  .",
        "   *  '  *",
        "   blossom",
    ],
    // summer: beach (sun + umbrella + waves)
    [
        "  \\ | /     ___",
        " -- O --   /___\\",
        "  / | \\      |",
        " ~~~~~~~~~~~~|~~~",
        "            |",
        "   beach",
    ],
    // autumn: falling leaves
    [
        "   ,  &   ,",
        "  &   ,  &  ,",
        " ,  &   ,   &",
        "   &   ,  &",
        "  ~~~~~~~~~~~",
        "   leaves",
    ],
    // winter: snowman
    [
        "     _===_",
        "    (.o.o.)",
        "    ( >^< )",
        "   (( : : ))",
        "  *  *  *  *  *",
        "   snowman",
    ],
];

/// The six-line scene tile for a season index (`0..=3`, clamped) — the last line
/// names the scene (blossom / beach / leaves / snowman).
#[must_use]
pub fn season_tile(season_index: u8) -> &'static [&'static str; 6] {
    &SEASON_TILE[(season_index as usize) % 4]
}

/// The scene tile for a season *name* (`spring`/`summer`/`autumn`/`winter`).
#[must_use]
pub fn season_tile_for(season: &str) -> &'static [&'static str; 6] {
    let idx = match season {
        "spring" => 0,
        "summer" => 1,
        "autumn" => 2,
        _ => 3, // winter
    };
    season_tile(idx)
}

use crate::cal::{season_for, Hemisphere, SeasonMarker};

/// The astronomical-event → hemisphere season name for a cardinal boundary, and
/// its title-cased form for the strip header.
fn season_title(solar_longitude_deg: f64, hemisphere: Hemisphere) -> &'static str {
    match season_for(solar_longitude_deg, hemisphere) {
        "spring" => "Spring",
        "summer" => "Summer",
        "autumn" => "Autumn",
        _ => "Winter",
    }
}

/// A year-long season timeline: the four astronomically-exact equinox/solstice
/// boundaries, each labelled with the season it opens (hemisphere-aware) and its
/// UTC date. Pure; spaces only (no box-drawing).
#[must_use]
pub fn season_strip(year: i16, markers: &[SeasonMarker], hemisphere: Hemisphere) -> String {
    use std::fmt::Write as _;
    let hemi = match hemisphere {
        Hemisphere::North => "N. hemisphere",
        Hemisphere::South => "S. hemisphere",
    };
    let mut out = format!("{year}  {hemi}\n");
    for m in markers {
        // "2026-06-21T08:24:30Z" -> date only.
        let date = m.instant_utc.split('T').next().unwrap_or(&m.instant_utc);
        let season = season_title(m.solar_longitude_deg, hemisphere);
        let _ = writeln!(
            out,
            "  {season:<7} opens {date}  ({} {:.0}deg)",
            m.term_name, m.solar_longitude_deg
        );
    }
    out
}
