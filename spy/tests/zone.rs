//! Tests for the display-zone parsing behind the overlay's footer control.
#![allow(clippy::unwrap_used)]

use timeglyph::{PosixNs, RenderZone};
use timeglyph_spy::zone::{self, ZoneChoice};

const WINTER: PosixNs = PosixNs(1_609_459_200_000_000_000); // 2021-01-01T00:00Z

#[test]
fn utc_is_the_calm_default() {
    for s in ["", "UTC", "utc", "Z"] {
        let z: ZoneChoice = zone::parse_zone(s).unwrap();
        assert!(matches!(z.zone, RenderZone::Utc), "{s:?}");
        assert_eq!(z.label, "UTC");
        assert!(!z.loud, "UTC must not be loud: {s:?}");
    }
}

#[test]
fn fixed_offset_parses_and_is_loud() {
    let z = zone::parse_zone("-05:00").unwrap();
    assert!(matches!(z.zone, RenderZone::Fixed(_)));
    assert!(z.loud);
}

#[test]
fn iana_name_parses_labelled_and_loud() {
    let z = zone::parse_zone("Asia/Shanghai").unwrap();
    assert!(matches!(z.zone, RenderZone::Named(_)));
    assert_eq!(z.label, "Asia/Shanghai");
    assert!(z.loud);
}

#[test]
fn local_keyword_resolves_to_host_zone() {
    let z = zone::parse_zone("local").unwrap();
    assert!(matches!(z.zone, RenderZone::Named(_)));
    assert_eq!(z.label, "Local");
    assert!(z.loud);
}

#[test]
fn unknown_zone_is_rejected_not_silently_utc() {
    assert!(zone::parse_zone("Nonsense/Zone").is_none());
    assert!(zone::parse_zone("+99:99").is_none());
}

#[test]
fn continents_and_zones_enumerate_the_iana_db() {
    let cs = zone::continents();
    for c in ["America", "Europe", "Asia"] {
        assert!(cs.iter().any(|x| x == c), "missing {c}: {cs:?}");
    }
    assert!(zone::zones_in("Europe")
        .iter()
        .any(|z| z == "Europe/London"));
}

#[test]
fn menu_label_is_windows_style_with_offset_and_abbr() {
    // Windows-style "(UTC−05:00) America/New_York · EST" — offset shown at selection.
    let l = zone::menu_label("America/New_York", WINTER);
    assert!(
        l.contains("America/New_York") && l.contains("-05") && l.contains("EST"),
        "{l}"
    );
}
