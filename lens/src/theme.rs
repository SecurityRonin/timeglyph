//! Overlay colour themes. The palette is data (testable): `tests/theme.rs` checks
//! every text tone clears WCAG AA contrast against the panel background, for both
//! the dark and the light palette.

use eframe::egui::Color32;

/// The selectable overlay theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Theme {
    /// Warm near-black (the default).
    #[default]
    Dark,
    /// Warm paper (light).
    Light,
}

/// A full overlay palette: backgrounds, hairline, and the semantic text tones —
/// `ink` = values, `amber` = accent, `mute` = labels, `faint` = captions,
/// `glyph` = the large empty-state mark. `base_dark` selects egui's Visuals base.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg_deep: Color32,
    pub bg_card: Color32,
    pub bg_chip: Color32,
    pub hairline: Color32,
    pub ink: Color32,
    pub amber: Color32,
    pub mute: Color32,
    pub faint: Color32,
    pub glyph: Color32,
    /// Confidence traffic-light dots: high (green), mid (amber), low (red).
    pub conf_high: Color32,
    pub conf_mid: Color32,
    pub conf_low: Color32,
    pub base_dark: bool,
}

impl Theme {
    /// The colours for this theme.
    #[must_use]
    pub fn palette(self) -> Palette {
        match self {
            Theme::Dark => DARK,
            Theme::Light => LIGHT,
        }
    }
}

/// The user's theme preference: follow the OS, or a fixed palette. Persisted;
/// `System` is the default, so a fresh install matches the OS light/dark setting
/// while a deliberate `Dark`/`Light` pick is remembered across sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ThemePreference {
    /// Follow the OS light/dark setting (the default).
    #[default]
    System,
    /// Always the dark palette.
    Dark,
    /// Always the light palette.
    Light,
}

impl ThemePreference {
    /// Resolve to a concrete [`Theme`] given the OS setting (`system`, `None` if
    /// unknown): `System` follows the OS and falls back to [`Theme::Dark`] when it
    /// is unknown; `Dark`/`Light` ignore the OS.
    #[must_use]
    pub fn resolve(self, system: Option<Theme>) -> Theme {
        match self {
            ThemePreference::System => system.unwrap_or(Theme::Dark),
            ThemePreference::Dark => Theme::Dark,
            ThemePreference::Light => Theme::Light,
        }
    }
}

/// Warm near-black palette. Text vs `bg_deep`: ink ~15:1, amber ~10:1, mute ~8:1,
/// faint ~5:1 — all clear WCAG AA.
pub const DARK: Palette = Palette {
    bg_deep: Color32::from_rgb(20, 18, 15), // warm near-black
    bg_card: Color32::from_rgb(31, 28, 22),
    bg_chip: Color32::from_rgb(38, 31, 20), // amber-tinted
    hairline: Color32::from_rgb(52, 47, 38),
    ink: Color32::from_rgb(245, 241, 232), // warm white — datetime values
    amber: Color32::from_rgb(240, 180, 41), // brass accent
    mute: Color32::from_rgb(179, 169, 145), // labels
    faint: Color32::from_rgb(143, 134, 116), // captions
    glyph: Color32::from_rgb(92, 82, 64),  // large empty-state mark
    conf_high: Color32::from_rgb(63, 185, 80), // green
    conf_mid: Color32::from_rgb(240, 180, 41), // amber
    conf_low: Color32::from_rgb(229, 83, 75), // red
    base_dark: true,
};

/// Warm paper palette. Dark tones on a warm off-white; the amber accent is
/// darkened to a bronze so it clears AA on the light ground (bright amber would
/// wash out).
pub const LIGHT: Palette = Palette {
    bg_deep: Color32::from_rgb(245, 241, 232), // warm off-white
    bg_card: Color32::from_rgb(236, 230, 217),
    bg_chip: Color32::from_rgb(240, 226, 197), // amber-tinted
    hairline: Color32::from_rgb(206, 196, 178),
    ink: Color32::from_rgb(28, 25, 20), // near-black — datetime values
    amber: Color32::from_rgb(138, 90, 0), // dark bronze accent (AA on light)
    mute: Color32::from_rgb(92, 84, 66), // labels
    faint: Color32::from_rgb(122, 112, 90), // captions
    glyph: Color32::from_rgb(201, 191, 168), // large empty-state mark
    conf_high: Color32::from_rgb(26, 127, 55), // dark green (visible on light)
    conf_mid: Color32::from_rgb(138, 90, 0), // bronze
    conf_low: Color32::from_rgb(196, 52, 43), // dark red
    base_dark: false,
};
