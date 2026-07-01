//! Context-aware plausibility components (ADR 0005): byte-width match, endian
//! match, artifact-context hint, and neighbour monotonicity.
//!
//! These four need information a bare integer lacks (the on-disk width/byte
//! order, an artifact source hint, sibling column values), so they live behind
//! an [`InterpretContext`]. The zero-context [`interpret_int`] is unchanged and
//! never emits them — they appear ONLY when their context is supplied, so the
//! safe default stays exactly as before.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret::{self, Endian, InterpretContext};

fn comp(cands: &[interpret::Candidate], id: &str, name: &str) -> Option<f64> {
    let c = cands.iter().find(|c| c.format_id == id)?;
    c.components
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| *v)
}

#[test]
fn zero_context_does_not_emit_the_contextual_components() {
    // The safe default: a bare interpret_int emits none of the four.
    let cands = interpret::interpret_int(1_577_836_800);
    for name in [
        "byte_width_match",
        "endian_match",
        "artifact_match",
        "neighbour_monotonicity",
    ] {
        assert!(
            comp(&cands, "unix", name).is_none(),
            "{name} must not appear without context"
        );
    }
}

#[test]
fn byte_width_match_rewards_the_formats_natural_width() {
    // FILETIME is a 64-bit (8-byte) field; an observed 8-byte width fits it.
    let ctx = InterpretContext {
        observed_width_bytes: Some(8),
        ..Default::default()
    };
    let cands = interpret::interpret_int_with_context(132_223_104_000_000_000, &ctx);
    assert_eq!(comp(&cands, "filetime", "byte_width_match"), Some(1.0));

    // Unix seconds is a classic 32-bit (4-byte) field. The same observed 8-byte
    // width is a partial fit (value would fit 4 bytes → plausibly zero-extended).
    let small = InterpretContext {
        observed_width_bytes: Some(8),
        ..Default::default()
    };
    let cands = interpret::interpret_int_with_context(1_577_836_800, &small);
    assert_eq!(comp(&cands, "unix", "byte_width_match"), Some(0.5));

    // A matching 4-byte width scores the unix reading a full 1.0.
    let exact = InterpretContext {
        observed_width_bytes: Some(4),
        ..Default::default()
    };
    let cands = interpret::interpret_int_with_context(1_577_836_800, &exact);
    assert_eq!(comp(&cands, "unix", "byte_width_match"), Some(1.0));
}

#[test]
fn endian_match_disambiguates_when_only_one_order_is_in_window() {
    // 1577836800 (0x5E0BE100) is an in-window 2020 unix-seconds value. The same
    // 4 bytes read in the OTHER order decode to a 1970-ish value (out of the
    // plausibility window), so this byte order is the disambiguated one → 1.0.
    let ctx = InterpretContext {
        observed_width_bytes: Some(4),
        endian: Some(Endian::Big),
        ..Default::default()
    };
    let cands = interpret::interpret_int_with_context(1_577_836_800, &ctx);
    assert_eq!(comp(&cands, "unix", "endian_match"), Some(1.0));

    // The byte-swapped value, under the same context, is the WRONG order: it is
    // out of window while its flip is in-window → endian_match 0.0.
    let swapped = i64::from((1_577_836_800u32).swap_bytes()); // 0x00E10B5E = 14748510
    let cands = interpret::interpret_int_with_context(swapped, &ctx);
    assert_eq!(comp(&cands, "unix", "endian_match"), Some(0.0));
}

#[test]
fn artifact_hint_matches_the_right_family() {
    // A "Google Chrome history" hint matches the Chrome/WebKit format, not unix.
    // Use a value that renders (to *some* civil date) under both, so both are
    // candidates and the artifact component is comparable across them.
    let ctx = InterpretContext {
        artifact: Some("Google Chrome history database"),
        ..Default::default()
    };
    let cands = interpret::interpret_int_with_context(1_577_836_800, &ctx);
    assert_eq!(comp(&cands, "webkit", "artifact_match"), Some(1.0));
    assert_eq!(
        comp(&cands, "unix", "artifact_match"),
        Some(0.2),
        "a chrome hint should not fully match the generic unix format"
    );
}

#[test]
fn neighbour_monotonicity_favours_a_coherent_column() {
    // A column of plainly-ordered unix-seconds values (all in-window, monotonic)
    // → the unix reading scores a full neighbour_monotonicity.
    let column = [
        1_577_836_800i64,
        1_577_836_860,
        1_577_923_200,
        1_580_000_000,
    ];
    let ctx = InterpretContext {
        neighbours: &column,
        ..Default::default()
    };
    let cands = interpret::interpret_int_with_context(column[0], &ctx);
    assert_eq!(comp(&cands, "unix", "neighbour_monotonicity"), Some(1.0));
}

#[test]
fn hex_path_emits_width_and_endian_components() {
    // The hex decoder knows the on-disk width AND byte order, so its candidates
    // must carry the byte_width_match + endian_match components.
    let v: u64 = 132_223_104_000_000_000; // 2020 FILETIME
    let le_hex = hex::encode(v.to_le_bytes());
    let groups = interpret::interpret_hex(&le_hex).unwrap();
    let (_, cands) = groups
        .iter()
        .find(|(layout, _)| layout == "u64 LE")
        .expect("u64 LE layout");
    let ft = cands
        .iter()
        .find(|c| c.format_id == "filetime")
        .expect("filetime candidate");
    assert!(ft.components.iter().any(|(n, _)| *n == "byte_width_match"));
    assert!(ft.components.iter().any(|(n, _)| *n == "endian_match"));
}
