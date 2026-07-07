//! On-disk persistence for the overlay's Settings.
//!
//! The overlay keeps its live preferences on `LensApp`; this module is the
//! testable serialization boundary (Humble Object). Settings persist to a JSON
//! file in the OS config dir so a prior session's display frame carries over —
//! a missing or corrupt file degrades to defaults (logged, never a panic).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use timeglyph::DateStyle;

use crate::theme::Theme;

/// The persisted overlay preferences: theme, whether the 干支 line shows, the
/// datetime display style, the default display-zone spec (as a `parse_zone`
/// string), and the optional 干支 longitude.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedSettings {
    /// The theme preference: `None` (default) follows the OS light/dark setting;
    /// `Some(Dark|Light)` is an explicit user choice, remembered across sessions.
    /// `#[serde(default)]` so a settings file predating this key loads as `None`.
    #[serde(default)]
    pub theme: Option<Theme>,
    /// Whether the 干支 / lunisolar line (and the longitude input) is shown.
    pub show_lunar: bool,
    /// Datetime display style for rendered readings.
    pub date_style: DateStyle,
    /// The default display-zone, as a `zone::parse_zone` spec (`UTC`, `local`,
    /// a fixed offset, or an IANA name).
    pub zone_spec: String,
    /// The 干支 hour-pillar longitude (°E), if set.
    pub longitude: Option<f64>,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            theme: None,
            show_lunar: false,
            date_style: DateStyle::default(),
            zone_spec: "UTC".to_string(),
            longitude: None,
        }
    }
}

/// The path of the settings file: `<config-dir>/timeglyph-lens/settings.json`
/// (via [`directories::ProjectDirs`]), falling back to
/// `~/.config/timeglyph-lens/settings.json`.
#[must_use]
pub fn config_path() -> Option<PathBuf> {
    if let Some(dirs) = directories::ProjectDirs::from("dev", "SecurityRonin", "timeglyph-lens") {
        return Some(dirs.config_dir().join("settings.json"));
    }
    // Fallback: the XDG default even when ProjectDirs can't resolve a base dir.
    directories::UserDirs::new().map(|d| d.home_dir().join(".config/timeglyph-lens/settings.json"))
}

/// Load persisted settings, degrading to [`PersistedSettings::default`] on a
/// missing or corrupt file (logged via `tracing`, never a panic).
#[must_use]
pub fn load() -> PersistedSettings {
    let Some(path) = config_path() else {
        tracing::debug!("no config dir resolved; using default settings");
        return PersistedSettings::default();
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(path = %path.display(), "no settings file; using defaults");
            return PersistedSettings::default();
        }
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "cannot read settings; using defaults");
            return PersistedSettings::default();
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "corrupt settings file; using defaults");
            PersistedSettings::default()
        }
    }
}

/// Persist `settings` to [`config_path`], creating the parent directory as
/// needed. A write failure is logged, never propagated as a panic — losing a
/// preference must not crash the overlay.
pub fn save(settings: &PersistedSettings) {
    let Some(path) = config_path() else {
        tracing::debug!("no config dir resolved; settings not saved");
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(path = %parent.display(), error = %e, "cannot create config dir; settings not saved");
            return;
        }
    }
    let json = match serde_json::to_string_pretty(settings) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(error = %e, "cannot serialize settings; not saved");
            return;
        }
    };
    if let Err(e) = std::fs::write(&path, json) {
        tracing::warn!(path = %path.display(), error = %e, "cannot write settings");
    }
}
