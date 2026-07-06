//! Ranking calibration against real labeled data (`tests/data/calibration.csv`):
//! for each value whose true format is known from the artifact it was carved
//! from, measure whether the ranking puts the true format in top-1 / top-3.
//! This is the measured basis for the scoring work (ADR-0005 successor) — a
//! Tier-1 accuracy number from real data, not a self-graded claim. The floor
//! assertion is a regression gate; the scoring priors should raise it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

struct Row {
    value: i64,
    format: String,
}

fn corpus() -> Vec<Row> {
    include_str!("data/calibration.csv")
        .lines()
        .skip(1) // header
        .filter_map(|l| {
            let mut it = l.split(',');
            let value = it.next()?.trim().parse().ok()?;
            let format = it.next()?.trim().to_string();
            Some(Row { value, format })
        })
        .collect()
}

/// Zero-based rank of `format` in the ranked readings for `value`, or `None` if
/// the true format is not among the readings at all.
fn rank_of(value: i64, format: &str) -> Option<usize> {
    interpret::interpret_int(value)
        .iter()
        .position(|c| c.format_id == format)
}

#[test]
fn calibration_accuracy_meets_floor() {
    let rows = corpus();
    assert!(!rows.is_empty(), "calibration corpus must load");
    let (mut top1, mut top3) = (0usize, 0usize);
    for r in &rows {
        match rank_of(r.value, &r.format) {
            Some(0) => {
                top1 += 1;
                top3 += 1;
            }
            Some(n) if n < 3 => top3 += 1,
            _ => {}
        }
    }
    let n = rows.len();
    let (p1, p3) = (top1 as f64 / n as f64, top3 as f64 / n as f64);
    println!(
        "calibration: n={n}  top-1={:.1}%  top-3={:.1}%",
        p1 * 100.0,
        p3 * 100.0
    );
    // Regression floors, set just under the measured baseline (top-1 26.2%,
    // top-3 100.0% on n=42) so a regression trips them. The prevalence prior
    // (next) should raise top-1 substantially while keeping top-3 at 100%.
    assert!(
        p3 >= 0.9,
        "top-3 accuracy {:.1}% below floor 90%",
        p3 * 100.0
    );
    assert!(
        p1 >= 0.2,
        "top-1 accuracy {:.1}% below floor 20%",
        p1 * 100.0
    );
}
