//! Persistence for the lens Settings (theme, 干支, DateStyle, default zone,
//! longitude). The GUI is the thin shell; this is the testable half.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::DateStyle;
use timeglyph_lens::settings::{CalendarVisibility, PersistedSettings};
use timeglyph_lens::theme::ThemePreference;
use timeglyph_lens::zone::parse_zone;

#[test]
fn default_theme_preference_is_system() {
    // No saved theme preference → System, which the overlay resolves to the OS
    // light/dark setting. A fixed Dark/Light pick persists that choice.
    let s = PersistedSettings::default();
    assert_eq!(s.theme, ThemePreference::System);
    assert!(!s.show_lunar);
    assert_eq!(s.date_style, DateStyle::Iso8601);
    assert_eq!(s.zone_spec, "UTC");
    assert_eq!(s.longitude, None);
    // Every alternative calendar is on by default.
    assert_eq!(s.calendars, CalendarVisibility::default());
    assert!(s.calendars.roc && s.calendars.japanese && s.calendars.persian);
}

#[test]
fn round_trips_through_json() {
    let s = PersistedSettings {
        theme: ThemePreference::Light,
        show_lunar: true,
        date_style: DateStyle::UsStyle,
        zone_spec: "Asia/Shanghai".to_string(),
        longitude: Some(120.5),
        calendars: CalendarVisibility {
            islamic: false,
            persian: false,
            ..CalendarVisibility::default()
        },
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: PersistedSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    assert!(!back.calendars.islamic && back.calendars.roc);
}

#[test]
fn missing_calendars_field_enables_all() {
    // A settings file predating the per-calendar toggles (no `calendars` key)
    // loads with every alternative calendar enabled — current behaviour preserved.
    let json = r#"{"show_lunar":true,"date_style":"Iso8601","zone_spec":"UTC","longitude":null}"#;
    let s: PersistedSettings = serde_json::from_str(json).unwrap();
    assert_eq!(s.calendars, CalendarVisibility::default());
    // The stable key → toggle mapping the renderer filters by (not the display
    // name, so relabelling never breaks a saved choice).
    assert!(s.calendars.shows("roc") && s.calendars.shows("persian"));
    let hidden = CalendarVisibility {
        japanese: false,
        ..CalendarVisibility::default()
    };
    assert!(!hidden.shows("japanese") && hidden.shows("hebrew"));
    // An unknown key defaults to shown.
    assert!(hidden.shows("gregorian"));
}

#[test]
fn missing_theme_field_deserializes_to_system() {
    // Forward/backward compatible: a settings file that predates the theme
    // preference (no `theme` key) loads as System → follow the OS, not a crash.
    let json = r#"{"show_lunar":false,"date_style":"Iso8601","zone_spec":"UTC","longitude":null}"#;
    let s: PersistedSettings = serde_json::from_str(json).unwrap();
    assert_eq!(s.theme, ThemePreference::System);
}

#[test]
fn a_prior_fixed_theme_string_still_loads() {
    // A settings file carrying a concrete "Dark"/"Light" (from an earlier build)
    // deserializes to that fixed preference, not a crash.
    for (raw, want) in [
        ("Dark", ThemePreference::Dark),
        ("Light", ThemePreference::Light),
    ] {
        let json = format!(
            r#"{{"theme":"{raw}","show_lunar":false,"date_style":"Iso8601","zone_spec":"UTC","longitude":null}}"#
        );
        let s: PersistedSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s.theme, want);
    }
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
