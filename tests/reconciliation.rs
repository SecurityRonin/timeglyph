//! Reconciliation: timeglyph's decode against externally-sourced ground truth.
//!
//! Runs always on a small COMMITTED sample of documented tier-1 values (time-decode
//! published examples, Discord's own docs, Apple TN1150) — so a decode regression
//! is caught in CI. It ADDITIONALLY reconciles a larger corpus pointed to by
//! `TIMEGLYPH_CORPUS_CSV`, the fleet pattern for real-world artifacts (large
//! corpora are gitignored + provided at runtime, per the test-data provenance
//! standard). Each row is `value,format,expected_utc` and the decode must render
//! with that UTC prefix.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::RenderZone;

fn reconcile(csv_text: &str, source: &str) {
    let mut checked = 0;
    for (i, line) in csv_text.lines().enumerate() {
        if i == 0 || line.trim().is_empty() {
            continue; // header / blank
        }
        let mut f = line.split(',');
        let value = f.next().unwrap().trim();
        let format = f.next().unwrap().trim();
        let expected = f.next().unwrap().trim();
        let inst = timeglyph::format(format)
            .unwrap_or_else(|_| panic!("{source} row {i}: unknown format {format:?}"))
            .decode_int(
                value
                    .parse()
                    .unwrap_or_else(|_| panic!("{source} row {i}: value {value:?} not i64")),
            )
            .unwrap_or_else(|e| panic!("{source} row {i}: decode {format} {value}: {e}"));
        let got = inst.render(&RenderZone::Utc).unwrap();
        assert!(
            got.starts_with(expected),
            "{source} row {i}: {format} {value} = {got:?}, expected prefix {expected:?}"
        );
        checked += 1;
    }
    assert!(checked > 0, "{source}: no rows reconciled");
}

#[test]
fn committed_sample_reconciles() {
    reconcile(include_str!("data/reconciliation.csv"), "committed sample");
}

#[test]
fn external_corpus_reconciles_when_present() {
    let Ok(path) = std::env::var("TIMEGLYPH_CORPUS_CSV") else {
        eprintln!("TIMEGLYPH_CORPUS_CSV not set — skipping external-corpus reconciliation");
        return;
    };
    let text = std::fs::read_to_string(&path).expect("read TIMEGLYPH_CORPUS_CSV");
    reconcile(&text, &path);
}
