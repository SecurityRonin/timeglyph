//! Plausibility-scoring component contract (HANDOFF §5b).
//!
//! Scoring must be a *named component set*, never a single opaque rank, so a
//! reviewer can see WHY a reading scored as it did. The load-bearing component
//! is `granularity_match`: a value with no sub-second resolution is a poor fit
//! for a sub-second unit (the classic seconds-vs-ms-vs-µs-vs-ns ambiguity).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

fn component(cands: &[interpret::Candidate], id: &str, name: &str) -> f64 {
    let c = cands
        .iter()
        .find(|c| c.format_id == id)
        .unwrap_or_else(|| panic!("no candidate {id}"));
    c.components
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("no component {name} on {id}"))
        .1
}

#[test]
fn every_candidate_emits_named_components() {
    let cands = interpret::interpret_int(1_577_836_800);
    let unix = cands.iter().find(|c| c.format_id == "unix").unwrap();
    for required in ["representable", "in_window", "granularity_match"] {
        assert!(
            unix.components.iter().any(|(n, _)| *n == required),
            "missing component {required}"
        );
    }
}

#[test]
fn whole_second_value_is_a_poor_fit_for_a_sub_second_unit() {
    // 1_577_836_800_000 ms == 2020-01-01T00:00:00.000 — in-window under unix_ms,
    // but it is exactly whole seconds, so granularity_match must be penalised.
    let round_ms = interpret::interpret_int(1_577_836_800_000);
    let g_round = component(&round_ms, "unix_ms", "granularity_match");
    assert!(
        g_round < 0.5,
        "whole-second ms granularity = {g_round}, want < 0.5"
    );

    // 1_577_836_801_501 ms == 2020-01-01T00:00:01.501 — carries real sub-second
    // resolution (no trailing zeros), so it fits the ms unit perfectly.
    let real_ms = interpret::interpret_int(1_577_836_801_501);
    let g_real = component(&real_ms, "unix_ms", "granularity_match");
    assert!(
        (g_real - 1.0).abs() < 1e-9,
        "sub-second ms granularity = {g_real}, want 1.0"
    );
}

#[test]
fn granularity_lifts_the_better_fitting_reading() {
    // Same magnitude, different sub-second content: the higher-resolution value
    // must score the ms reading at least as high as the whole-second value does.
    let real = interpret::interpret_int(1_577_836_801_501);
    let round = interpret::interpret_int(1_577_836_800_000);
    let s_real = real
        .iter()
        .find(|c| c.format_id == "unix_ms")
        .unwrap()
        .score;
    let s_round = round
        .iter()
        .find(|c| c.format_id == "unix_ms")
        .unwrap()
        .score;
    assert!(
        s_real > s_round,
        "real-resolution {s_real} should beat round {s_round}"
    );
}

#[test]
fn id_schemes_do_not_outrank_a_plain_unix_seconds_value() {
    // A 10-digit unix-seconds value, read as a Snowflake, lands essentially AT
    // the scheme's own epoch (id >> 22 ≈ 0) — implausible. magnitude_fit must
    // sink the ID readings below the plain unix-seconds reading (ranked, never
    // hidden).
    let cands = interpret::interpret_int(1_577_836_800);
    let unix = cands.iter().find(|c| c.format_id == "unix").unwrap().score;
    for id in ["snowflake", "discord"] {
        if let Some(c) = cands.iter().find(|c| c.format_id == id) {
            assert!(
                unix > c.score,
                "{id} ({}) must not outrank unix ({unix})",
                c.score
            );
        }
    }
}

#[test]
fn real_discord_id_surfaces_a_confident_discord_reading() {
    let cands = interpret::interpret_int(175_928_847_299_117_063);
    let d = cands
        .iter()
        .find(|c| c.format_id == "discord")
        .expect("discord candidate");
    assert!(d.rendered.as_deref().unwrap().starts_with("2016-04-30"));
    assert_eq!(component(&cands, "discord", "in_window"), 1.0);
    assert!(
        component(&cands, "discord", "magnitude_fit") > 0.5,
        "a real id sits well past the epoch → high magnitude_fit"
    );
}
