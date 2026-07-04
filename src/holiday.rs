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
    TABLE.get_or_init(|| decode().unwrap_or_default())
}

fn decode() -> Option<Table> {
    let mut gz = flate2::read::GzDecoder::new(RAW);
    let mut json = String::new();
    gz.read_to_string(&mut json).ok()?;
    serde_json::from_str(&json).ok()
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
    ZONES.get_or_init(|| serde_json::from_str(RAW).unwrap_or_default())
}
