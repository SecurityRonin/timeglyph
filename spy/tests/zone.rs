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

#[test]
fn etc_gmt_labels_show_the_true_offset() {
    // POSIX Etc/GMT sign is inverted: Etc/GMT-8 is 8h EAST of UTC = UTC+08:00.
    assert_eq!(zone::clean_label("Etc/GMT-8"), "UTC+08:00");
    assert_eq!(zone::clean_label("Etc/GMT+5"), "UTC-05:00");
    assert_eq!(zone::clean_label("Etc/GMT-14"), "UTC+14:00");
    assert_eq!(zone::clean_label("Etc/GMT"), "UTC");
    assert_eq!(zone::clean_label("Etc/GMT0"), "UTC");
}

#[test]
fn normal_zone_names_pass_through_clean_label() {
    assert_eq!(zone::clean_label("Asia/Shanghai"), "Asia/Shanghai");
    assert_eq!(zone::clean_label("America/New_York"), "America/New_York");
}

#[test]
fn parse_zone_relabels_etc_gmt_to_its_offset() {
    let z = zone::parse_zone("Etc/GMT-8").unwrap();
    assert_eq!(
        z.label, "UTC+08:00",
        "footer chip must not show the reversed id"
    );
}

#[test]
fn picker_lists_etc_but_not_systemv() {
    // Etc is restored (its menu is deduped + offset-sorted); SystemV stays out —
    // its aliases aren't tidied and it's obscure.
    let cs = zone::continents();
    assert!(cs.iter().any(|c| c == "Etc"), "Etc restored: {cs:?}");
    assert!(
        !cs.iter().any(|c| c == "SystemV"),
        "SystemV stays out: {cs:?}"
    );
    assert!(cs.iter().any(|c| c == "America"), "real regions present");
}

#[test]
fn etc_utc_aliases_clean_to_plain_utc() {
    for n in [
        "Etc/UTC",
        "Etc/Zulu",
        "Etc/Greenwich",
        "Etc/Universal",
        "Etc/UCT",
    ] {
        assert_eq!(zone::clean_label(n), "UTC", "{n}");
    }
}

#[test]
fn etc_menu_entries_are_deduped_and_offset_sorted() {
    let entries = zone::menu_entries("Etc", WINTER);
    let labels: Vec<String> = entries.iter().map(|(_, l)| l.clone()).collect();
    // No duplicate labels (the old bug was many identical "UTC").
    let mut uniq = labels.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), labels.len(), "duplicate labels: {labels:?}");
    assert!(
        labels.iter().any(|l| l == "UTC"),
        "has plain UTC: {labels:?}"
    );
    // Offset-sorted: a negative offset precedes a positive one.
    let neg = labels.iter().position(|l| l.contains("-12:00"));
    let pos = labels.iter().position(|l| l.contains("+12:00"));
    if let (Some(a), Some(b)) = (neg, pos) {
        assert!(a < b, "not offset-sorted: {labels:?}");
    }
}

#[test]
fn zone_summary_does_not_double_an_offset_label() {
    // A cleaned Etc/GMT zone's label already IS the offset — don't repeat it.
    let z = zone::parse_zone("Etc/GMT-8").unwrap();
    let s = zone::zone_summary(&z, WINTER);
    assert!(s.contains("UTC+08:00"), "{s}");
    assert!(!s.contains("· UTC+08"), "must not double the offset: {s}");
}

#[test]
fn zone_summary_is_label_abbr_equals_offset_without_a_warning() {
    // "Asia/Shanghai (CST = UTC+08:00)" — no caution sign, abbr = offset in parens.
    let z = zone::parse_zone("Asia/Shanghai").unwrap();
    let s = zone::zone_summary(&z, WINTER);
    assert!(!s.contains('⚠'), "no caution sign: {s}");
    assert!(s.starts_with("Asia/Shanghai ("), "{s}");
    assert!(s.contains("CST = UTC+08"), "{s}");
    // Winter London is GMT.
    let l = zone::zone_summary(&zone::parse_zone("Europe/London").unwrap(), WINTER);
    assert!(l.contains("(GMT = UTC"), "{l}");
}

#[test]
fn zone_summary_appends_offset_for_named_zones() {
    let z = zone::parse_zone("Asia/Shanghai").unwrap();
    let s = zone::zone_summary(&z, WINTER);
    assert!(s.contains("Asia/Shanghai") && s.contains("UTC+08"), "{s}");
}

#[test]
fn offset_spec_formats_hours_as_signed_hhmm() {
    // The single offset formatter shared by clean_label and the map.
    assert_eq!(zone::offset_spec(-5.0), "-05:00");
    assert_eq!(zone::offset_spec(5.5), "+05:30");
    assert_eq!(zone::offset_spec(14.0), "+14:00");
    assert_eq!(zone::offset_spec(0.0), "UTC");
}

#[test]
fn local_zone_summary_names_the_resolved_zone() {
    // "Local" alone doesn't say which zone — surface the resolved IANA name:
    // "Local (Asia/Shanghai (CST) = UTC+08:00)".
    let z = ZoneChoice {
        zone: RenderZone::parse("Asia/Shanghai").unwrap(),
        label: "Local".to_string(),
        loud: true,
    };
    let s = zone::zone_summary(&z, WINTER);
    assert!(s.starts_with("Local ("), "{s}");
    assert!(s.contains("Asia/Shanghai (CST)"), "{s}");
    assert!(s.contains("= UTC+08"), "{s}");
    assert!(!s.contains('⚠'), "{s}");
}

#[test]
fn continent_label_lowercases_etc() {
    assert_eq!(zone::continent_label("Etc"), "etc.");
    assert_eq!(zone::continent_label("America"), "America");
}
