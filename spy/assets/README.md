# Bundled assets — provenance

## `timezones.json` — world time-zone regions (for the clickable map)

- **Source:** Natural Earth, *Time Zones* (`ne_10m_time_zones`), via the
  `nvkelso/natural-earth-vector` GeoJSON mirror:
  `https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_time_zones.geojson`
- **Downloaded:** 2026-07-02
- **Source SHA-256:** `aa52ab97e5f906693f31fe2e625c9d6be78a4e74fcd4142a7261235b355cacfe`
- **License:** **Public domain.** Natural Earth: "All versions of Natural Earth
  raster and vector map data found on this website are in the public domain." No
  attribution or redistribution restrictions — safe to bundle.
- **Processing:** each feature simplified with Shapely 2.1
  (`buffer(0).simplify(0.25, preserve_topology=True)`), exterior rings only,
  coordinates rounded to 2 decimals (≈1 km). 3.8 MB / 155,007 vertices →
  120 regions / 128 rings / 5,029 vertices (~76 KB).
- **Schema:** a JSON array of `{ "o": <utc-offset hours>, "z": <IANA name|null>,
  "r": [ [ [lon, lat], … ], … ] }`. `z` is Natural Earth's `tz_name1st` (a
  *representative* IANA name for the offset band, present for 87/120 regions);
  the map is offset-keyed, not exact-IANA — the Continent → Zone dropdown remains
  the precise IANA selector.
- **Consumed by:** `src/tzmap.rs` (`regions()`, `zone_at()`).
