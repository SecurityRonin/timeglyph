//! Epistemic-framing contract (HANDOFF §5c).
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
