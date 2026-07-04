//! Each reading carries the engine's plausibility score so the overlay can show
//! a confidence percentage per candidate. The score is the same `[0, 1]` value
//! `timeglyph::interpret::Candidate::score` exposes; `confidence_pct` renders it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::scan;
use timeglyph::RenderZone;

#[test]
fn readings_carry_a_unit_range_confidence_score() {
    // 1577836800 = Unix seconds 2020-01-01, in-window → at least a `unix` reading.
    let readings = scan::readings_for("1577836800", 4, &RenderZone::Utc);
    assert!(!readings.is_empty(), "expected in-window readings");
    for r in &readings {
        assert!(
            (0.0..=1.0).contains(&r.score),
            "score {} for {} not in [0,1]",
            r.score,
            r.format_id
        );
    }
}

#[test]
fn confidence_pct_rounds_and_clamps() {
    assert_eq!(scan::confidence_pct(0.0), 0);
    assert_eq!(scan::confidence_pct(1.0), 100);
    assert_eq!(scan::confidence_pct(0.756), 76);
    assert_eq!(scan::confidence_pct(1.5), 100, "above 1.0 clamps to 100");
    assert_eq!(scan::confidence_pct(-0.2), 0, "below 0.0 clamps to 0");
}
