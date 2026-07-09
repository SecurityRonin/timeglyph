//! Auto-identification of floating-point timestamps.
//!
//! Some epochs are stored as a `double`, not an integer — Cocoa / CFAbsoluteTime
//! (WhatsApp-iOS `ZWAMESSAGE.ZMESSAGEDATE`, e.g. `608322295.31165`), OLE
//! automation dates, Julian / Modified-Julian day numbers. A fractional literal
//! cannot be an integer epoch, so the integer decoders are structurally
//! inapplicable; `interpret_float` reports the `LinearFloat`-strategy readings
//! instead, preserving the sub-second fraction the integer path would truncate.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

#[test]
fn fractional_cocoa_is_identified_with_subsecond_precision() {
    // 606940977.71577 s since 2001 == 2020-03-26T18:42:57.716Z — the fraction
    // (~716 ms) must survive, which the integer `cocoa` decoder cannot express.
    let cands = interpret::interpret_float(606_940_977.715_77);
    let c = cands
        .iter()
        .find(|c| c.format_id == "cocoa_float")
        .expect("cocoa_float candidate");
    let r = c.rendered.as_deref().expect("rendered");
    assert!(
        r.starts_with("2020-03-26T18:42:57.7"),
        "sub-second precision lost: {r}"
    );
    assert!(
        (c.score - 1.0).abs() < 1e-9,
        "in-window cocoa_float score {}",
        c.score
    );
}

#[test]
fn float_path_yields_only_float_strategy_formats() {
    // The integer-only decoders (unix, filetime, cocoa, …) never appear on the
    // float path — only the LinearFloat strategies can decode a double.
    let cands = interpret::interpret_float(608_322_295.311_65);
    assert!(!cands.is_empty(), "expected at least cocoa_float");
    assert!(
        cands.iter().all(|c| matches!(
            c.format_id,
            "cocoa_float" | "unix_float" | "ole" | "sqlite_julian" | "excel1904" | "mjd"
        )),
        "a non-float-strategy format leaked onto the float path: {:?}",
        cands.iter().map(|c| c.format_id).collect::<Vec<_>>()
    );
    assert!(cands.iter().any(|c| c.format_id == "cocoa_float"));
}

#[test]
fn out_of_civil_range_float_yields_no_reading() {
    // A giant magnitude must render nowhere civil and be dropped — never a
    // saturated, plausible-looking date.
    let cands = interpret::interpret_float(1.0e30);
    assert!(
        cands.is_empty(),
        "expected no readings, got {:?}",
        cands.iter().map(|c| c.format_id).collect::<Vec<_>>()
    );
}

#[test]
fn non_finite_float_yields_no_reading() {
    assert!(interpret::interpret_float(f64::NAN).is_empty());
    assert!(interpret::interpret_float(f64::INFINITY).is_empty());
}

#[test]
fn tied_float_readings_sort_deterministically_by_id() {
    // A small day-count value lands several LinearFloat formats in civil range
    // at once (cocoa_float, ole, excel1904 all score 1.0), exercising the tie
    // comparator. The tie-break is prevalence-desc THEN id: the common
    // cocoa_float/ole (prevalence 1.0) precede the niche excel1904 (0.5), and
    // within a prevalence tier the order is alphabetical — deterministic.
    let tied = || -> Vec<&'static str> {
        interpret::interpret_float(43900.5)
            .into_iter()
            .filter(|c| (c.score - 1.0).abs() < 1e-9)
            .map(|c| c.format_id)
            .collect()
    };
    let t = tied();
    assert!(t.len() >= 2, "need >=2 tied readings: {t:?}");
    if let (Some(ole), Some(ex)) = (
        t.iter().position(|&x| x == "ole"),
        t.iter().position(|&x| x == "excel1904"),
    ) {
        assert!(
            ole < ex,
            "common `ole` must precede niche `excel1904` in the tie: {t:?}"
        );
    }
    assert_eq!(t, tied(), "tie order must be deterministic");
}
