//! Epistemic-framing contract (ADR 0005).
//!
//! A forensic reading is *evidence*, not a verdict. The engine must frame every
//! candidate as "consistent with" a format — never "detected"/"is" — and must
//! carry the leap-smear disclaimer for POSIX-labelled readings (a raw value
//! cannot reveal whether its source clock smeared leap seconds).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

#[test]
fn candidates_are_framed_as_consistent_with_not_a_verdict() {
    let cands = interpret::interpret_int(1_577_836_800);
    let unix = cands.iter().find(|c| c.format_id == "unix").unwrap();
    let joined = unix.assumptions.join(" ").to_lowercase();
    assert!(
        joined.contains("consistent with"),
        "assumptions must frame the reading as 'consistent with': {:?}",
        unix.assumptions
    );
    assert!(
        !joined.contains("detected"),
        "must not use verdict language ('detected'): {:?}",
        unix.assumptions
    );
}

#[test]
fn local_time_formats_surface_a_no_offset_caveat() {
    // 1_391_422_645 is a valid FAT/DOS packed value (2021-07-15). FAT stores
    // LOCAL wall-clock time with no offset, so the reading must say so — the
    // rendered instant is naive, not UTC.
    let cands = interpret::interpret_int(1_391_422_645);
    let fat = cands
        .iter()
        .find(|c| c.format_id == "fat")
        .expect("fat candidate");
    let joined = fat.assumptions.join(" ").to_lowercase();
    assert!(
        joined.contains("naive") && joined.contains("offset"),
        "a FAT reading must surface its no-offset / naive caveat: {:?}",
        fat.assumptions
    );
}

#[test]
fn posix_readings_carry_a_leap_smear_disclaimer() {
    let cands = interpret::interpret_int(1_577_836_800);
    let unix = cands.iter().find(|c| c.format_id == "unix").unwrap();
    let joined = unix.assumptions.join(" ").to_lowercase();
    assert!(
        joined.contains("leap"),
        "a POSIX reading must carry the leap-smear disclaimer: {:?}",
        unix.assumptions
    );
}
