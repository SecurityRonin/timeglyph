//! Whole-world public-holiday lookup, behind the `holiday` feature.
//!
//! The data is an embedded, gzipped export of the MIT-licensed
//! [python-holidays](https://github.com/vacanza/holidays) project covering
//! 1980–2100 (see `data/generate_holidays.py` and `data/README.md`). Names are
//! in each country's default locale, exactly as python-holidays emits them.
//!
//! Epistemics: a hit means the date *is consistent with a public holiday* in
//! that country per the reference data — an annotation, not proof the day was
//! observed at a given place. Coverage varies by country (python-holidays
//! supports different year ranges per locale), so a `None` means "no holiday in
//! the covered data", not "provably an ordinary day".
//!
//! Cost: the ~1.5 MB blob is inflated and parsed once, lazily, on the first
//! lookup (an [`OnceLock`]); the parsed table is then held for the process
//! lifetime (tens of MB). Both are opt-in with the feature.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

use jiff::civil::Date;

use crate::RenderZone;

/// The embedded dataset: gzip of `{ country: { "YYYY-MM-DD": name } }`.
static RAW: &[u8] = include_bytes!("../data/holidays.json.gz");

/// country (ISO-3166 alpha-2) → (ISO date → holiday name).
type Table = HashMap<String, HashMap<String, String>>;

/// Inflate + parse the embedded blob once. A decode failure degrades to an empty
/// table (no holidays) rather than panicking — the blob is embedded and
/// CI-validated (tests/holiday.rs asserts it loads with a realistic country
/// count), so a runtime failure here means a packaging regression, caught there.
fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| load_or_empty("holidays", decode(RAW)))
}

/// Degrade a decode `Result` to `T::default()` with a LOUD log on failure, shared
/// by `table()` and `zones()` (DRY). The embedded blobs are CI-validated, so a
/// runtime failure means a packaging regression — logged at error level, never a
/// silent empty table that reads like "no holidays".
fn load_or_empty<T: Default>(what: &str, decoded: Result<T, String>) -> T {
    decoded.unwrap_or_else(|e| {
        tracing::error!(table = what, error = %e, "embedded holiday data failed to decode; degrading to empty");
        T::default()
    })
}

fn decode(raw: &[u8]) -> Result<Table, String> {
    let mut gz = flate2::read::GzDecoder::new(raw);
    let mut json = String::new();
    gz.read_to_string(&mut json)
        .map_err(|e| format!("gzip inflate of holidays.json.gz failed: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("JSON parse of holiday table failed: {e}"))
}

/// Public-holiday name for `date` in `country` (ISO-3166 alpha-2, case-insensitive),
/// or `None` when that date is not a holiday there / is outside the country's
/// covered range.
#[must_use]
pub fn lookup(country: &str, date: Date) -> Option<String> {
    // jiff renders civil dates as zero-padded ISO 8601 (YYYY-MM-DD), matching
    // the dataset's keys.
    let key = date.to_string();
    table().get(&country.to_uppercase())?.get(&key).cloned()
}

/// Number of countries in the embedded dataset. Lets a caller report coverage
/// and lets CI assert the blob loaded (a truncated/missing blob → 0).
#[must_use]
pub fn supported_country_count() -> usize {
    table().len()
}

/// The ISO-3166 alpha-2 country an IANA time-zone name belongs to (e.g.
/// `Asia/Shanghai` → `CN`) — how a caller turns a display zone into the country
/// whose holidays annotate a reading. `None` for `Etc/*` / `UTC` and unknown
/// zones (no single country → no holiday annotation).
#[must_use]
pub fn country_for_zone(iana: &str) -> Option<&'static str> {
    zones().get(iana).map(String::as_str)
}

/// Holiday name for `date` as interpreted in `zone`: resolves the zone's IANA
/// name → ISO country ([`country_for_zone`]) → [`lookup`]. `None` unless `zone`
/// is a named IANA zone mapping to a country — so UTC and fixed-offset display
/// zones never annotate (there is no single country to attribute the day to).
#[must_use]
pub fn in_zone(zone: &RenderZone, date: Date) -> Option<String> {
    let RenderZone::Named(tz) = zone else {
        return None;
    };
    lookup(country_for_zone(tz.iana_name()?)?, date)
}

/// Like [`in_zone`] but taking a reading's *rendered* datetime string, using its
/// leading ISO date (`YYYY-MM-DD`) — so callers that already hold the rendered
/// value (the CLI, the overlay) need not depend on jiff to parse it.
#[must_use]
pub fn in_zone_rendered(zone: &RenderZone, rendered: &str) -> Option<String> {
    let date = rendered.get(..10)?.parse().ok()?;
    in_zone(zone, date)
}

/// IANA zone → ISO-3166 alpha-2, from the tz database's `zone.tab` (public
/// domain). Small (~10 KB), parsed once. See `data/zone_country.json`.
fn zones() -> &'static HashMap<String, String> {
    static ZONES: OnceLock<HashMap<String, String>> = OnceLock::new();
    static RAW: &str = include_str!("../data/zone_country.json");
    ZONES.get_or_init(|| load_or_empty("zone→country", decode_zones(RAW)))
}

fn decode_zones(raw: &str) -> Result<HashMap<String, String>, String> {
    serde_json::from_str(raw).map_err(|e| format!("JSON parse of zone→country table failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_rejects_non_gzip_input() {
        let err = decode(b"this is plainly not a gzip stream").unwrap_err();
        assert!(err.contains("gzip inflate"), "unexpected error: {err}");
    }

    #[test]
    fn decode_rejects_valid_gzip_with_bad_json() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(b"{ not valid json").unwrap();
        let gz = enc.finish().unwrap();
        let err = decode(&gz).unwrap_err();
        assert!(err.contains("JSON parse"), "unexpected error: {err}");
    }

    #[test]
    fn decode_zones_rejects_bad_json() {
        let err = decode_zones("{ not valid json").unwrap_err();
        assert!(err.contains("JSON parse"), "unexpected error: {err}");
    }

    #[test]
    fn load_or_empty_degrades_loudly_on_err() {
        // The loud-degrade path shared by table()/zones(): a decode failure must
        // yield an empty default, not propagate — exercised here for real (the
        // embedded blobs never fail, so this is the only way to reach the arm).
        // A subscriber is installed so tracing actually formats the error field
        // (tracing is lazy — without an interested subscriber the event is a no-op).
        let sub = tracing_subscriber::fmt()
            .with_writer(std::io::sink)
            .finish();
        let degraded: Table = tracing::subscriber::with_default(sub, || {
            load_or_empty("test", Err("simulated corruption".to_string()))
        });
        assert!(degraded.is_empty());
    }

    #[test]
    fn load_or_empty_passes_ok_through() {
        let mut m = Table::new();
        m.insert("XX".to_string(), HashMap::new());
        let kept = load_or_empty("test", Ok(m));
        assert_eq!(kept.len(), 1);
    }
}
