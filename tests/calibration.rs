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

/// Per-FAMILY reliability — never one global "0.9 = 90%". A single global top-3
/// (~92%) HIDES that some families' true *format label* is systematically
/// out-ranked. This reports each family's top-1/top-3 and gates it: every
/// well-sampled family (n≥5) must clear an 80% top-3 floor, EXCEPT families
/// explicitly acknowledged below — where the decoded INSTANT is still correct but
/// a more-common format sharing that instant/window wins the label
/// (`active`↔`filetime` are the same FILETIME instant; `dttm`/`sqlserver` share
/// crowded windows). A NEW family dropping below the floor fails here, forcing a
/// conscious scoring fix or an explicit acknowledgement — low families are
/// surfaced, never averaged away.
#[test]
fn per_family_reliability_is_reported_and_floored() {
    use std::collections::{BTreeMap, BTreeSet};
    use timeglyph::format;

    // Families whose true LABEL sits below the floor because the INSTANT is right
    // but a more-common same-instant / same-window format out-ranks the label.
    // The decoded time is still correct — only the provenance label is demoted;
    // the tool shows every reading, so the true format is present, just not top-3.
    const ACKNOWLEDGED_LOW: &[&str] = &[
        // `active` (AD FILETIME) is the SAME instant as `filetime`; the ubiquitous
        // `filetime` label wins the prevalence tie-break. Same time, common label.
        "Active Directory, LDAP (lastLogon, pwdLastSet)",
        // `dotnet_ticks` shares its 100 ns-tick window with `filetime`/`active`.
        ".NET / SQL Server datetime2",
        // niche packed `dvr` shares a crowded seconds/ms window with mainstream formats.
        "DVR WFS / DHFS filesystems",
        // packed civil `dttm` is out-ranked by linear formats sharing its window.
        "Microsoft Compound File / Office DTTM",
        // niche packed `nokiale` shares a crowded window with common formats.
        "Nokia devices",
    ];

    let rows = corpus();
    let mut fam: BTreeMap<&'static str, (usize, usize, usize)> = BTreeMap::new();
    for r in &rows {
        let family = format(&r.format).map_or("?", |f| f.family);
        let e = fam.entry(family).or_insert((0, 0, 0));
        e.0 += 1;
        match rank_of(&r.value, &r.format) {
            Some(0) => {
                e.1 += 1;
                e.2 += 1;
            }
            Some(n) if n < 3 => e.2 += 1,
            _ => {}
        }
    }
    assert!(
        fam.len() >= 20,
        "reliability must be per-family, not global: only {} families measured",
        fam.len()
    );

    let ack: BTreeSet<&str> = ACKNOWLEDGED_LOW.iter().copied().collect();
    println!("{:46} {:>4} {:>6} {:>6}", "family", "n", "top1", "top3");
    let mut unacknowledged_low = Vec::new();
    for (f, (n, t1, t3)) in &fam {
        let p3 = *t3 as f64 / *n as f64;
        println!(
            "{f:46} {n:>4} {:>5.0}% {:>5.0}%",
            *t1 as f64 / *n as f64 * 100.0,
            p3 * 100.0
        );
        if *n >= 5 && p3 < 0.80 && !ack.contains(f) {
            unacknowledged_low.push(*f);
        }
    }
    assert!(
        unacknowledged_low.is_empty(),
        "families below 80% top-3 that are not acknowledged (fix the scoring, or record \
         why the label is legitimately out-ranked): {unacknowledged_low:?}"
    );
}
