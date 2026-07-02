//! Tests for the display-zone parsing behind the overlay's footer control.
#![allow(clippy::unwrap_used)]

use timeglyph::RenderZone;
use timeglyph_spy::zone::{self, ZoneChoice};

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
