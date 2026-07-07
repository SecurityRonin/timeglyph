//! Persistence for the lens Settings (theme, 干支, DateStyle, default zone,
//! longitude). The GUI is the thin shell; this is the testable half.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::DateStyle;
use timeglyph_lens::settings::PersistedSettings;
use timeglyph_lens::theme::Theme;
use timeglyph_lens::zone::parse_zone;

#[test]
fn default_theme_is_none_meaning_follow_system() {
    // No saved theme preference → None, which the overlay resolves to the OS
    // light/dark setting. Only an explicit user choice persists a concrete theme.
    let s = PersistedSettings::default();
    assert_eq!(s.theme, None);
    assert!(!s.show_lunar);
    assert_eq!(s.date_style, DateStyle::Iso8601);
    assert_eq!(s.zone_spec, "UTC");
    assert_eq!(s.longitude, None);
}

#[test]
fn round_trips_through_json() {
    let s = PersistedSettings {
        theme: Some(Theme::Light),
        show_lunar: true,
        date_style: DateStyle::UsStyle,
        zone_spec: "Asia/Shanghai".to_string(),
        longitude: Some(120.5),
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: PersistedSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn missing_theme_field_deserializes_to_follow_system() {
    // Forward/backward compatible: a settings file that predates the theme
    // preference (no `theme` key) loads as None → follow the system, not a crash.
    let json = r#"{"show_lunar":false,"date_style":"Iso8601","zone_spec":"UTC","longitude":null}"#;
    let s: PersistedSettings = serde_json::from_str(json).unwrap();
    assert_eq!(s.theme, None);
}

#[test]
fn corrupt_json_deserializes_to_error_not_panic() {
    // A caller feeds this to `load`, which maps the error to defaults.
    let bad = "{ this is not json";
    assert!(serde_json::from_str::<PersistedSettings>(bad).is_err());
}

#[test]
fn persisted_zone_spec_initializes_a_zone_choice() {
    let s = PersistedSettings {
        zone_spec: "Asia/Shanghai".to_string(),
        ..PersistedSettings::default()
    };
    let zc = parse_zone(&s.zone_spec).expect("known IANA zone parses");
    assert_eq!(zc.label, "Asia/Shanghai");
    assert!(zc.loud);
}

#[test]
fn default_zone_spec_initializes_utc() {
    let s = PersistedSettings::default();
    let zc = parse_zone(&s.zone_spec).expect("UTC parses");
    assert_eq!(zc.label, "UTC");
    assert!(!zc.loud);
}
