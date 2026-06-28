//! Spec-anchored correctness tests (clean-room, independent anchors).
//!
//! Each anchor is a fact from a primary source, not derived from our own math:
//! - a format's value `0` MUST render to that format's documented epoch date;
//! - the canonical FILETIME→Unix anchor (116444736000000000 == 1970-01-01);
//! - a widely-published Unix anchor (1577836800 == 2020-01-01).
//!
//! HANDOFF §"Validation": extend into a differential battery against the MIT
//! `time_decode` oracle + each spec's worked example.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::{format, interpret};

/// Render `value` under format `id` and assert it starts with `prefix`.
fn assert_decodes(id: &str, value: i64, prefix: &str) {
    let f = format(id).unwrap();
    let inst = f.decode_int(value).unwrap();
    let rendered = inst.to_rfc3339().unwrap_or_default();
    assert!(
        rendered.starts_with(prefix),
        "{id}({value}) = {rendered:?}, expected to start {prefix:?}"
    );
}

#[test]
fn value_zero_renders_each_formats_epoch() {
    assert_decodes("unix", 0, "1970-01-01T00:00:00");
    assert_decodes("webkit", 0, "1601-01-01T00:00:00"); // µs since 1601
    assert_decodes("cocoa", 0, "2001-01-01T00:00:00"); // s since 2001
    assert_decodes("hfsplus", 0, "1904-01-01T00:00:00"); // s since 1904
    assert_decodes("dotnet_ticks", 0, "0001-01-01T00:00:00"); // 100ns since 0001
}

#[test]
fn canonical_filetime_unix_anchor() {
    // The published anchor: FILETIME 116444736000000000 == Unix epoch.
    assert_decodes("filetime", 116_444_736_000_000_000, "1970-01-01T00:00:00");
}

#[test]
fn published_unix_anchor_2020() {
    assert_decodes("unix", 1_577_836_800, "2020-01-01T00:00:00");
    // FILETIME for the same instant (cross-format consistency).
    assert_decodes("filetime", 132_223_104_000_000_000, "2020-01-01T00:00:00");
}

#[test]
fn filetime_round_trips() {
    let f = format("filetime").unwrap();
    let v = 132_223_104_000_000_000i64;
    let inst = f.decode_int(v).unwrap();
    assert_eq!(f.encode_int(inst).unwrap(), v, "decode∘encode is identity");
}

#[test]
fn ole_float_epoch() {
    // OLE Automation: 0.0 days == 1899-12-30 (the documented serial-date base).
    let f = format("ole").unwrap();
    let inst = f.decode_float(0.0).unwrap();
    assert!(inst
        .to_rfc3339()
        .unwrap()
        .starts_with("1899-12-30T00:00:00"));
}

#[test]
fn interpret_is_multi_candidate_never_single_answer() {
    // A bare value is ambiguous: the 2020 Unix-seconds value is ALSO a plausible
    // reading under other epochs. The engine must surface several candidates, and
    // the unix reading must be present and in-window.
    let cands = interpret::interpret_int(1_577_836_800);
    assert!(
        cands.len() >= 2,
        "a raw value is underdetermined — expected multiple candidates, got {}",
        cands.len()
    );
    let unix = cands
        .iter()
        .find(|c| c.format_id == "unix")
        .expect("unix candidate");
    assert!(unix.rendered.as_deref().unwrap().starts_with("2020-01-01"));
    assert!(
        unix.score > 0.0,
        "the 2020 unix reading is in the plausible window"
    );
    // Epistemics: every candidate carries its assumptions + scored components.
    assert!(!unix.assumptions.is_empty() && !unix.components.is_empty());
}

#[test]
fn hex_decode_le_be() {
    // FILETIME for 2020-01-01 = 132223104000000000. Derive its little-endian
    // bytes (so the literal can't drift) and assert they decode back to a 2020
    // FILETIME candidate.
    let v: u64 = 132_223_104_000_000_000;
    let le_hex = hex::encode(v.to_le_bytes());
    let groups = interpret::interpret_hex(&le_hex).unwrap();
    let le_u64 = groups
        .iter()
        .find(|(layout, _)| layout == "u64 LE")
        .expect("u64 LE layout present");
    assert!(
        le_u64.1.iter().any(|c| c.format_id == "filetime"
            && c.rendered
                .as_deref()
                .unwrap_or("")
                .starts_with("2020-01-01")),
        "LE bytes should yield a 2020 FILETIME candidate"
    );
}
