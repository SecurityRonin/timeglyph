//! Plausibility-scoring component contract (ADR 0005).
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
    assert!((component(&cands, "discord", "in_window") - 1.0).abs() < 1e-9);
    assert!(
        component(&cands, "discord", "magnitude_fit") > 0.5,
        "a real id sits well past the epoch → high magnitude_fit"
    );
}

// --- epoch_distance prior (MAGNITUDE/RECENCY) --------------------------------
// A real timestamp's magnitude places the decoded instant WELL PAST the
// format's own epoch, not hugging it. A reading that lands minutes/hours after
// a format's epoch (because the value is orders of magnitude too small for that
// unit) is weak evidence for that format. This is a low-weight prior: it nudges
// the rank, never hides a reading (see interpret::score_components).

#[test]
fn every_candidate_emits_the_epoch_distance_component() {
    let cands = interpret::interpret_int(1_577_836_800);
    let unix = cands.iter().find(|c| c.format_id == "unix").unwrap();
    assert!(
        unix.components.iter().any(|(n, _)| *n == "epoch_distance"),
        "epoch_distance must be a visible, named component on every candidate"
    );
}

#[test]
fn epoch_hugging_reading_scores_low_epoch_distance() {
    // 1487100001000 is a real 2017 Unix-ms value. Read as iostime (ns since
    // 2001) it decodes to 2001-01-01 + ~24 minutes — essentially AT the iostime
    // epoch, which is implausibly small for a ns-since-2001 value (a real one is
    // ~17-19 digits). So iostime's epoch_distance must be ~0, while unix_ms
    // (2017, decades past its 1970 epoch) must be ~1.
    let cands = interpret::interpret_int(1_487_100_001_000);
    assert!(
        component(&cands, "iostime", "epoch_distance") < 0.05,
        "an epoch-hugging iostime reading must score ~0 epoch_distance"
    );
    assert!(
        component(&cands, "unix_ms", "epoch_distance") > 0.95,
        "a value decades past the unix epoch must score ~1 epoch_distance"
    );
}

#[test]
fn epoch_distance_lifts_the_true_format_above_an_epoch_hugger() {
    // The whole point: the true unix_ms reading (13-digit 2017 ms) must rank #1,
    // ABOVE iostime which only wins by hugging its 2001 epoch and sorting first
    // alphabetically. This is the top-1 win the prior exists to deliver.
    let cands = interpret::interpret_int(1_487_100_001_000);
    assert_eq!(
        cands[0].format_id, "unix_ms",
        "unix_ms must rank #1 for a real 2017 Unix-ms value, not the epoch-hugging iostime"
    );
}

#[test]
fn webkit_outranks_epoch_hugging_iostime() {
    // 13224789208197989 is a real Chrome/WebKit (µs since 1601) 2020 value. Read
    // as iostime it lands mid-2001 (only ~months past the 2001 epoch), so webkit
    // — decades past its own epoch — must rank #1.
    let cands = interpret::interpret_int(13_224_789_208_197_989);
    assert_eq!(
        cands[0].format_id, "webkit",
        "webkit must rank #1 for a real 2020 Chrome µs value, not iostime"
    );
}

#[test]
fn genuine_early_epoch_reading_still_appears() {
    // Epistemics: the prior LOWERS rank, it NEVER hides a reading. A genuine
    // cocoa value one hour past the 2001 epoch (small, epoch-hugging) must still
    // appear as a candidate — just ranked lower — never filtered out.
    let cands = interpret::interpret_int(3_600); // cocoa: 2001-01-01T01:00:00Z
    let cocoa = cands.iter().find(|c| c.format_id == "cocoa");
    assert!(
        cocoa.is_some(),
        "a genuine early-epoch cocoa reading must still appear (ranked, not hidden)"
    );
    assert!(
        component(&cands, "cocoa", "epoch_distance") < 0.05,
        "an hour past the epoch scores low epoch_distance — but is still surfaced"
    );
}

fn pos_int(value: i64, id: &str) -> usize {
    interpret::interpret_int(value)
        .iter()
        .position(|c| c.format_id == id)
        .unwrap_or_else(|| panic!("no candidate {id} for {value}"))
}

#[test]
fn every_candidate_emits_the_prevalence_component() {
    let cands = interpret::interpret_int(1_577_836_800);
    assert!(
        cands[0].components.iter().any(|(n, _)| *n == "prevalence"),
        "prevalence component must be emitted"
    );
}

#[test]
fn prevalence_ranks_ubiquitous_filetime_above_niche_active() {
    // A real FILETIME (100 ns since 1601) also decodes in-window as AD `active`
    // (identical encoding), so they tie on every other component. FILETIME is
    // ubiquitous in evidence; AD `active` is niche — the prevalence prior must
    // rank filetime first (previously lost the alphabetical tie-break to active).
    assert!(
        pos_int(132_963_748_799_479_404, "filetime") < pos_int(132_963_748_799_479_404, "active"),
        "filetime must outrank the niche `active` on a tie"
    );
}

#[test]
fn prevalence_ranks_ubiquitous_ole_above_niche_excel1904() {
    // OLE automation date (ubiquitous in Windows/Office) vs Excel-1904 (rare
    // legacy Mac Excel) — a value valid as both must rank ole first.
    let cands = interpret::interpret_float(42_073.333_333_333_336);
    let ole = cands.iter().position(|c| c.format_id == "ole").unwrap();
    let excel = cands
        .iter()
        .position(|c| c.format_id == "excel1904")
        .unwrap();
    assert!(
        ole < excel,
        "ole {ole} must outrank excel1904 {excel} on a tie"
    );
}
