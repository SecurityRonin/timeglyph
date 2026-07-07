//! Ranking calibration against real labeled data (`tests/data/calibration.csv`):
//! for each value whose true format is known from the artifact it was carved
//! from, measure whether the ranking puts the true format in top-1 / top-3.
//! This is the measured basis for the scoring work (ADR-0005 successor) — a
//! Tier-1 accuracy number from real data, not a self-graded claim. The floor
//! assertion is a regression gate; the scoring priors should raise it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

struct Row {
    value: String,
    format: String,
}

fn corpus() -> Vec<Row> {
    include_str!("data/calibration.csv")
        .lines()
        .skip(1) // header
        .filter_map(|l| {
            let mut it = l.split(',');
            let value = it.next()?.trim().to_string();
            let format = it.next()?.trim().to_string();
            (!value.is_empty()).then_some(Row { value, format })
        })
        .collect()
}

/// Zero-based rank of `format` among the readings for `value`. Integer values go
/// through interpret_int; a fractional literal (OLE/Excel/Julian/Cocoa-double)
/// through interpret_float — mirroring the CLI auto path.
fn rank_of(value: &str, format: &str) -> Option<usize> {
    let cands = if let Ok(v) = value.parse::<i64>() {
        interpret::interpret_int(v)
    } else if let Ok(v) = value.parse::<f64>() {
        interpret::interpret_float(v)
    } else {
        return None;
    };
    cands.iter().position(|c| c.format_id == format)
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
        distinct.len() >= 28,
        "corpus should span >=28 formats (has {}: {:?})",
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
    // Include packed/bit-field formats so the corpus validates the full packed
    // tier: FAT/exFAT, BCD/semi-octet telephony, Nokia/Motorola/Symantec
    // hardware clocks, GSM network time, SQL Server datetime, and MJD.
    for fmt in [
        "fat",
        "exfat",
        "bcd",
        "sqlserver",
        "gsm",
        "moto",
        "symantec",
        "nokiale",
        "ns40",
        "logtime",
    ] {
        assert!(
            distinct.contains(fmt),
            "corpus should include the packed format {fmt}"
        );
    }
}

#[test]
fn calibration_accuracy_meets_floor() {
    let rows = corpus();
    assert!(!rows.is_empty(), "calibration corpus must load");
    let (mut top1, mut top3) = (0usize, 0usize);
    for r in &rows {
        match rank_of(&r.value, &r.format) {
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
    // Regression floors. Baseline was top-1 27.1% before any prior; the
    // epoch_distance (MAGNITUDE/RECENCY) prior lifted it to 57.7% by demoting
    // epoch-huggers (a 13-digit Unix-ms read as iostime = 2001 + minutes), and
    // the prevalence TIE-BREAK (score-neutral; demotes only the rare long tail so
    // e.g. filetime beats AD `active`) lifted it to 60.1% — all by RE-ORDERING,
    // never hiding: every reading is still shown, likelihood is information not a
    // filter. Current measured: top-1 60.1%, top-3 92.0% (n=336) — adding the
    // niche dhcp6 + classic-hfs formats trims top-3 slightly (they legitimately
    // enter some top-3 lists), the honest cost of more valid interpretations.
    // Floors sit just under, so a regression trips them; top-3 must stay high
    // (the true format is only re-ordered, never dropped from contention).
    assert!(
        p3 >= 0.90,
        "top-3 accuracy {:.1}% below floor 90% (the prior must not drop true formats)",
        p3 * 100.0
    );
    assert!(
        p1 >= 0.55,
        "top-1 accuracy {:.1}% below floor 55%",
        p1 * 100.0
    );
}
