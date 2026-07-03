//! The bundled world time-zone map: Natural Earth (public domain) regions,
//! simplified for a clickable overlay. Pure geometry (point-in-polygon → zone);
//! the egui map is the shell over this. Provenance: `assets/README.md`.

use std::sync::OnceLock;

use serde::Deserialize;

/// One time-zone region: a UTC offset (whole/fractional hours), an optional
/// representative IANA name (present for ~⅔ of regions; `None` for ocean /
/// offset-only bands), and exterior rings of `[lon, lat]` degrees.
#[derive(Debug, Clone, Deserialize)]
pub struct Region {
    #[serde(rename = "o")]
    pub offset: f64,
    #[serde(rename = "z")]
    pub iana: Option<String>,
    #[serde(rename = "r")]
    pub rings: Vec<Vec<[f32; 2]>>,
}

/// The zone a clicked point resolves to.
#[derive(Debug, Clone)]
pub struct ZonePick {
    /// UTC offset in hours (e.g. `-5.0`).
    pub offset: f64,
    /// A representative IANA name for the region, if Natural Earth has one.
    pub iana: Option<String>,
}

static REGIONS: OnceLock<Vec<Region>> = OnceLock::new();

/// The bundled regions (parsed once from the embedded asset).
#[must_use]
pub fn regions() -> &'static [Region] {
    REGIONS
        .get_or_init(|| {
            serde_json::from_str(include_str!("../assets/timezones.json"))
                .expect("bundled timezones.json is valid at build time")
        })
        .as_slice()
}

/// Resolve a map click at (`lon`, `lat`) in degrees to a zone, or `None` when the
/// point is outside every region (off-map) or not finite.
#[must_use]
pub fn zone_at(lon: f32, lat: f32) -> Option<ZonePick> {
    if !lon.is_finite() || !lat.is_finite() {
        return None;
    }
    regions()
        .iter()
        .find(|r| r.rings.iter().any(|ring| point_in_ring(lon, lat, ring)))
        .map(|r| ZonePick {
            offset: r.offset,
            iana: r.iana.clone(),
        })
}

/// Even-odd (ray-casting) point-in-polygon test for one ring.
fn point_in_ring(x: f32, y: f32, ring: &[[f32; 2]]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let [xi, yi] = ring[i];
        let [xj, yj] = ring[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}
