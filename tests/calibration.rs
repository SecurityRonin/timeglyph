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
fn corpus_covers_diverse_epoch_families() {
    // Beyond the initial unix_ms/cocoa/webkit, the corpus must carry the epoch
    // families the ranking most confuses — Unix seconds, FILETIME (100 ns since
    // 1601), and iostime (ns since 2001) — so a magnitude/recency prior can be
    // validated across formats rather than over-fit to three.
    let rows = corpus();
    for fmt in ["unix", "filetime", "iostime"] {
        let n = rows.iter().filter(|r| r.format == fmt).count();
        assert!(n >= 5, "corpus needs >=5 {fmt} values (has {n})");
    }
    // Breadth: the ranking must be measured across many epoch families, not a
    // handful, so a scoring prior can't be tuned to a narrow set. Generated
    // tier-1 values (time-decode --timestamp) broaden coverage beyond the
    // artifact-carved formats.
    let distinct: std::collections::BTreeSet<&str> =
        rows.iter().map(|r| r.format.as_str()).collect();
    assert!(
        distinct.len() >= 19,
        "corpus should span >=19 formats (has {}: {:?})",
        distinct.len(),
        distinct
    );
    // Include float-strategy formats (OLE / Excel / Julian / Cocoa-double), which
    // only decode via interpret_float — so the harness must handle them too.
    for fmt in ["ole", "excel1904", "sqlite_julian", "cocoa_float"] {
        assert!(
            distinct.contains(fmt),
            "corpus should include the float-strategy format {fmt}"
        );
    }
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
    // Regression floors, set just under the measured baseline on the DIVERSE
    // corpus: top-1 13.4%, top-3 91.5% (n=82). The narrow 3-format corpus read
    // 26%/100% — optimistic; adding filetime/iostime/unix exposed that only
    // `cocoa` reliably ranks #1 (the 18–19-digit 100 ns formats and the ms/µs
    // families cross-tie). A magnitude/recency prior should raise top-1; these
    // floors trip on a regression.
    assert!(
        p3 >= 0.85,
        "top-3 accuracy {:.1}% below floor 85%",
        p3 * 100.0
    );
    assert!(
        p1 >= 0.10,
        "top-1 accuracy {:.1}% below floor 10%",
        p1 * 100.0
    );
}
