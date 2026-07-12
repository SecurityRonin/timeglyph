//! Machine-checked per-format validation-tier manifest (calibration foundation).
//!
//! `docs/validation.md` *claims* tier-3 is avoided; this test makes that claim a
//! GATE instead of prose. The manifest (`docs/validation-tiers.tsv`) records, per
//! value-producing format, its highest-trust independent oracle and tier. The
//! assertions below prove:
//!   1. every registry format is audited (no un-audited format can ship);
//!   2. none is tier-3-only (every format has a tier-1/tier-2 independent anchor —
//!      not merely a self-authored round-trip, which is the LZNT1-trap zone);
//!   3. each tier claim is BOUND to reality — the oracle file the manifest names
//!      actually references the format (so the manifest cannot claim an oracle it
//!      does not have);
//!   4. formats that lean on a single independent oracle are a tracked, shrinking
//!      allowlist — a new format cannot silently ship single-anchored.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use timeglyph::registry::FORMATS;

/// The oracle test files a manifest row may cite, included so a citation can be
/// verified against the file's actual contents (the binding check).
const ORACLE_FILES: &[(&str, &str)] = &[
    ("oracle.rs", include_str!("oracle.rs")),
    ("oracle_crates.rs", include_str!("oracle_crates.rs")),
    ("oracle_extra.rs", include_str!("oracle_extra.rs")),
    ("unfurl_oracle.rs", include_str!("unfurl_oracle.rs")),
    ("catalog.rs", include_str!("catalog.rs")),
    ("anchors.rs", include_str!("anchors.rs")),
    ("floats.rs", include_str!("floats.rs")),
    ("packed.rs", include_str!("packed.rs")),
    ("extra.rs", include_str!("extra.rs")),
];

struct Row {
    tier: u8,
    primary_file: String,
    has_second: bool,
}

/// Parse `docs/validation-tiers.tsv` → id → row. The `*_oracle` columns look like
/// `time-decode(oracle.rs)`; the filename in parens is the binding target.
fn manifest() -> BTreeMap<String, Row> {
    let tsv = include_str!("../docs/validation-tiers.tsv");
    let file_of = |cell: &str| -> Option<String> {
        let start = cell.find('(')? + 1;
        let end = cell.find(')')?;
        Some(cell[start..end].to_string())
    };
    tsv.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            let primary_file = file_of(c[2]).unwrap_or_default();
            let has_second = c.get(3).is_some_and(|s| !s.trim().is_empty());
            (
                c[0].to_string(),
                Row {
                    tier: c[1].parse().unwrap(),
                    primary_file,
                    has_second,
                },
            )
        })
        .collect()
}

#[test]
fn every_registry_format_is_audited_and_vice_versa() {
    let m = manifest();
    let registry: BTreeSet<&str> = FORMATS.iter().map(|f| f.id).collect();
    let audited: BTreeSet<&str> = m.keys().map(String::as_str).collect();
    let missing: Vec<_> = registry.difference(&audited).collect();
    let phantom: Vec<_> = audited.difference(&registry).collect();
    assert!(
        missing.is_empty(),
        "registry formats not in the manifest: {missing:?}"
    );
    assert!(
        phantom.is_empty(),
        "manifest lists non-registry formats: {phantom:?}"
    );
}

#[test]
fn no_format_is_tier_3_only() {
    for (id, row) in manifest() {
        assert!(
            row.tier <= 2,
            "{id} is tier-{} — a value-producing format may not rest on a self-authored \
             round-trip alone (LZNT1-trap zone); add a tier-1/2 independent anchor",
            row.tier
        );
    }
}

#[test]
fn every_tier_claim_is_bound_to_the_oracle_file_that_references_the_format() {
    for (id, row) in manifest() {
        let content = ORACLE_FILES
            .iter()
            .find(|(name, _)| *name == row.primary_file)
            .unwrap_or_else(|| {
                panic!(
                    "{id}: manifest cites unknown oracle file {:?}",
                    row.primary_file
                )
            })
            .1;
        assert!(
            content.contains(&format!("\"{id}\"")),
            "{id}: manifest cites {} as its oracle, but that file never references \"{id}\" \
             — the tier claim is unbacked",
            row.primary_file
        );
    }
}

#[test]
fn single_anchor_formats_are_a_tracked_shrinking_allowlist() {
    // Formats validated by exactly ONE independent oracle. Adding a second
    // independent anchor (a spec worked example is tool-independent tier-1)
    // removes a format from this list; a NEW format must not land here silently.
    // This set may only SHRINK.
    const KNOWN_SINGLE_ANCHOR: &[&str] = &[
        "bcd",
        "bitdate",
        "bitdec",
        "cocoa",
        "dhcp6",
        "dttm",
        "exfat",
        "gmsgid",
        "gsm",
        "hfs",
        "logtime",
        "nokiale",
        "postgres",
        "semioctet",
        "sony",
        "sqlserver",
        "unix_ms",
        "unix_ns",
        "unix_us",
    ];
    let known: BTreeSet<&str> = KNOWN_SINGLE_ANCHOR.iter().copied().collect();
    let actual: BTreeSet<String> = manifest()
        .into_iter()
        .filter(|(_, r)| !r.has_second)
        .map(|(id, _)| id)
        .collect();
    let untracked: Vec<_> = actual
        .iter()
        .filter(|id| !known.contains(id.as_str()))
        .collect();
    assert!(
        untracked.is_empty(),
        "formats leaning on a single oracle but not in the tracked allowlist: {untracked:?} \
         — give them a second independent anchor, or (deliberately) add them to the list"
    );
}
