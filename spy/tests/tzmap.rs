//! Tests for the bundled time-zone map geometry (point-in-polygon → zone).
#![allow(clippy::unwrap_used)]

use timeglyph_spy::tzmap;

#[test]
fn zone_at_resolves_known_cities() {
    let ny = tzmap::zone_at(-73.97, 40.75).unwrap();
    assert_eq!(ny.offset, -5.0);
    assert_eq!(ny.iana.as_deref(), Some("America/New_York"));

    let ldn = tzmap::zone_at(-0.12, 51.50).unwrap();
    assert_eq!(ldn.offset, 0.0);
    assert_eq!(ldn.iana.as_deref(), Some("Europe/London"));

    let la = tzmap::zone_at(-118.24, 34.05).unwrap();
    assert_eq!(la.offset, -8.0);
}

#[test]
fn zone_at_off_map_is_none_not_panic() {
    assert!(tzmap::zone_at(999.0, 999.0).is_none());
    assert!(tzmap::zone_at(f32::NAN, 0.0).is_none());
}

#[test]
fn regions_are_bundled() {
    assert!(tzmap::regions().len() >= 100, "expected the full NE set");
    assert!(tzmap::regions().iter().all(|r| !r.rings.is_empty()));
}
