//! Display-zone parsing for the overlay's footer control. Pure and testable; the
//! egui footer is the thin shell over this.

use timeglyph::{PosixNs, RenderZone};

use crate::tzinfo;

/// A parsed display zone plus how to present it in the footer chip.
#[derive(Debug, Clone)]
pub struct ZoneChoice {
    /// The zone readings are rendered in.
    pub zone: RenderZone,
    /// Short label for the footer chip (`UTC`, `Local`, `Asia/Shanghai`, …).
    pub label: String,
    /// True for any non-UTC zone. The footer renders a loud zone prominently so a
    /// glance cannot mistake the frame for UTC (a classic timestamp-reading error).
    pub loud: bool,
}

impl Default for ZoneChoice {
    fn default() -> Self {
        Self {
            zone: RenderZone::Utc,
            label: "UTC".to_string(),
            loud: false,
        }
    }
}

/// Parse a zone spec into a [`ZoneChoice`], or `None` if unrecognised.
///
/// `""` / `UTC` / `Z` → UTC (the calm default); `local` / `system` → the host's
/// zone; a leading `+`/`-` → a fixed offset; anything else → an IANA name,
/// validated against the tz database (an unknown name is rejected, never a silent
/// UTC fallback).
#[must_use]
pub fn parse_zone(input: &str) -> Option<ZoneChoice> {
    let s = input.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("utc") || s.eq_ignore_ascii_case("z") {
        return Some(ZoneChoice::default());
    }
    if s.eq_ignore_ascii_case("local") || s.eq_ignore_ascii_case("system") {
        return Some(ZoneChoice {
            zone: RenderZone::Named(jiff::tz::TimeZone::system()),
            label: "Local".to_string(),
            loud: true,
        });
    }
    let zone = RenderZone::parse(s).ok()?;
    Some(ZoneChoice {
        zone,
        label: s.to_string(),
        loud: true,
    })
}

/// All IANA zone names known to jiff.
fn available() -> impl Iterator<Item = String> {
    jiff::tz::db().available().map(|n| n.as_str().to_string())
}

/// The distinct continents/areas in the IANA database (first path segment),
/// sorted — the first level of the hierarchical picker.
#[must_use]
pub fn continents() -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();
    for name in available() {
        if let Some((head, _)) = name.split_once('/') {
            set.insert(head.to_string());
        }
    }
    set.into_iter().collect()
}

/// The full IANA zone names under `continent` (e.g. `Europe` → `Europe/London`),
/// sorted — the second level of the picker.
#[must_use]
pub fn zones_in(continent: &str) -> Vec<String> {
    let prefix = format!("{continent}/");
    let mut v: Vec<String> = available().filter(|n| n.starts_with(&prefix)).collect();
    v.sort();
    v
}

/// A Windows-style menu label: `(UTC-05:00) America/New_York · EST`, with the
/// offset and abbreviation resolved at `at`. A location alone is ambiguous, so
/// the offset is shown at selection time.
#[must_use]
pub fn menu_label(name: &str, at: PosixNs) -> String {
    match RenderZone::parse(name) {
        Ok(zone) => match tzinfo::stamp(&zone, at) {
            Some(s) => {
                let abbr = if s.abbr.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", s.abbr)
                };
                format!("(UTC{}) {name}{abbr}", s.offset)
            }
            None => format!("(UTC) {name}"),
        },
        Err(_) => name.to_string(),
    }
}
