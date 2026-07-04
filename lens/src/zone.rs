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

/// Format a UTC offset in hours as the `parse_zone` spec form — `±HH:MM`, or
/// `UTC` for zero. `-5.0` → `-05:00`, `5.5` → `+05:30`, `0.0` → `UTC`. The single
/// offset formatter shared by [`clean_label`] and the map.
#[must_use]
pub fn offset_spec(hours: f64) -> String {
    if hours == 0.0 {
        return "UTC".to_string();
    }
    let sign = if hours < 0.0 { '-' } else { '+' };
    let a = hours.abs();
    let h = a.trunc() as u32;
    let m = ((a - a.trunc()) * 60.0).round() as u32;
    format!("{sign}{h:02}:{m:02}")
}

/// A human display label for a zone name. The POSIX `Etc/GMT±N` ids invert the
/// sign (`Etc/GMT-8` is 8h *east* = UTC+08:00), which misleads, so they are
/// rewritten to their true offset (via [`offset_spec`]); every other name passes
/// through unchanged.
#[must_use]
pub fn clean_label(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("Etc/") {
        // Etc/* are non-geographic offset aliases; present them as their offset.
        if let Some(gmt) = rest.strip_prefix("GMT") {
            if let Ok(n) = gmt.parse::<i32>() {
                // POSIX Etc/GMT sign is inverted; offset_spec gives "UTC" for zero.
                let spec = offset_spec(f64::from(-n));
                return if spec == "UTC" {
                    spec
                } else {
                    format!("UTC{spec}")
                };
            }
        }
        // Etc/GMT, Etc/UTC, Etc/UCT, Etc/Universal, Etc/Zulu, Etc/Greenwich → UTC.
        return "UTC".to_string();
    }
    name.to_string()
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
        label: clean_label(s),
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
    // SystemV is a bag of sign-inverted offset aliases that clean_label doesn't
    // tidy — excluded. Etc is kept: its menu is cleaned by menu_entries (deduped,
    // offset-sorted) and clean_label rewrites its ids to plain offsets.
    const PSEUDO: &[&str] = &["SystemV"];
    let mut set = std::collections::BTreeSet::new();
    for name in available() {
        if let Some((head, _)) = name.split_once('/') {
            if !PSEUDO.contains(&head) {
                set.insert(head.to_string());
            }
        }
    }
    set.into_iter().collect()
}

/// The active-zone summary for the footer chip (no caution sign — the always-amber
/// chip and the explicit offset already flag the frame; offset/abbr/DST resolved
/// at `at`):
/// - `UTC` and offset-only labels (fixed / cleaned `Etc/GMT`) stand alone;
/// - a named zone: `Asia/Shanghai (CST = UTC+08:00)`, or `Asia/Kolkata (UTC+05:30)`
///   when the zone has no letter code;
/// - `Local`: the resolved system zone is surfaced —
///   `Local (Asia/Shanghai (HKT) = UTC+08:00)`.
#[must_use]
pub fn zone_summary(zone: &ZoneChoice, at: PosixNs) -> String {
    // A label that is already an offset (fixed offset, or a cleaned Etc/GMT zone)
    // stands alone — nothing to add.
    let label_is_offset =
        zone.label.starts_with("UTC") || zone.label.starts_with('+') || zone.label.starts_with('-');
    match tzinfo::stamp(&zone.zone, at) {
        Some(_) if label_is_offset => zone.label.clone(),
        Some(s) => {
            let dst = if s.dst { " ☀ DST" } else { "" };
            // What identifies the zone inside the parens: for Local, the resolved
            // system-zone name (+ abbr); otherwise just the abbr (the label already
            // names the zone).
            let ident = if zone.label == "Local" {
                match iana_name(&zone.zone) {
                    Some(n) if !s.abbr.is_empty() => format!("{n} ({})", s.abbr),
                    Some(n) => n,
                    None => s.abbr.clone(),
                }
            } else {
                s.abbr.clone()
            };
            if ident.is_empty() {
                format!("{} (UTC{}{dst})", zone.label, s.offset)
            } else {
                format!("{} ({ident} = UTC{}{dst})", zone.label, s.offset)
            }
        }
        None => zone.label.clone(),
    }
}

/// The IANA name of a zone, if it is a named zone (e.g. `Local` → `Asia/Shanghai`).
fn iana_name(zone: &RenderZone) -> Option<String> {
    match zone {
        RenderZone::Named(tz) => tz.iana_name().map(str::to_string),
        _ => None,
    }
}

/// The picker display name for a continent path-segment. `Etc` — a non-geographic
/// offset-alias bag — is shown as `etc.`; every real region passes through.
#[must_use]
pub fn continent_label(continent: &str) -> String {
    if continent == "Etc" {
        "etc.".to_string()
    } else {
        continent.to_string()
    }
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

/// The `(zone-name, display-label)` submenu entries for `continent`, sorted by
/// UTC offset (at `at`) then name and deduplicated by label. This collapses the
/// Etc region's many UTC aliases into a single `UTC` entry and orders its bands
/// `UTC-12:00 → UTC+14:00`, rather than the lexical `Etc/GMT+1, +10, +11…` mess.
#[must_use]
pub fn menu_entries(continent: &str, at: PosixNs) -> Vec<(String, String)> {
    let mut rows: Vec<(f64, String, String)> = zones_in(continent)
        .into_iter()
        .map(|z| {
            let off = RenderZone::parse(&z)
                .ok()
                .and_then(|rz| tzinfo::offset_hours(&rz, at))
                .unwrap_or(0.0);
            let label = menu_label(&z, at);
            (off, z, label)
        })
        .collect();
    rows.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    let mut seen = std::collections::HashSet::new();
    rows.into_iter()
        .filter(|(_, _, label)| seen.insert(label.clone()))
        .map(|(_, z, label)| (z, label))
        .collect()
}

/// A Windows-style menu label: `(UTC-05:00) America/New_York · EST`, with the
/// offset and abbreviation resolved at `at`. A location alone is ambiguous, so
/// the offset is shown at selection time.
#[must_use]
pub fn menu_label(name: &str, at: PosixNs) -> String {
    // A sign-inverted Etc/GMT id is already its own offset once cleaned — show it
    // once rather than "(UTC+08:00) Etc/GMT-8".
    let clean = clean_label(name);
    if clean != name {
        return clean;
    }
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
