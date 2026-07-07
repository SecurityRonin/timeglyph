//! Timestomp annotation contract.
//!
//! A Windows **FILETIME** value whose sub-second 100 ns field is exactly zero is
//! a soft forensic signal of `SetFileTime`-style manipulation: the Windows file
//! API typically sets whole-second precision when called directly, whereas a
//! naturally-recorded file time almost never lands on a clean second boundary.
//!
//! Deliberately scoped to `filetime` ONLY. AD `active` (Integer8) shares the
//! 100 ns-since-1601 encoding, but many AD attributes (`accountExpires`,
//! `pwdLastSet`, `lockoutTime`) are *legitimately* whole-second or coarser and
//! are not set via `SetFileTime` — annotating them would be a false positive and
//! an overstatement, which a forensic tool must not do.
//!
//! The engine annotates this as an **assumption**, framed "consistent with" —
//! never a verdict. A non-zero sub-second field must NOT trigger the note.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

// A filetime value that decodes to exactly 2020-01-01T00:00:00Z (whole second,
// zero sub-second 100 ns ticks). Computed as:
//   (2020-01-01T00:00:00Z - 1601-01-01) in 100 ns units
//   = (1577836800 + 11644473600) × 10_000_000 = 132225120000000000
const FILETIME_WHOLE_SECOND: i64 = 132_225_120_000_000_000;

// A filetime value that decodes to 2020-01-01T00:00:00.0000001Z (one 100 ns
// tick of sub-second — NOT a whole second). = FILETIME_WHOLE_SECOND + 1
const FILETIME_SUBSEC_NONZERO: i64 = 132_225_120_000_000_001;

#[test]
fn filetime_zero_subsecond_carries_timestomp_annotation() {
    let cands = interpret::interpret_int(FILETIME_WHOLE_SECOND);
    let ft = cands
        .iter()
        .find(|c| c.format_id == "filetime")
        .expect("filetime candidate must be present");
    let joined = ft.assumptions.join(" ").to_lowercase();
    assert!(
        joined.contains("consistent with") && joined.contains("manipulation"),
        "a filetime with zero sub-second field must carry a 'consistent with … manipulation' assumption; got: {:?}",
        ft.assumptions
    );
    // The framing must say "consistent with", never assert a verdict.
    assert!(
        !joined.contains("was timestomped") && !joined.contains("is timestomped"),
        "must NOT use verdict language ('was timestomped' / 'is timestomped'): {:?}",
        ft.assumptions
    );
}

#[test]
fn filetime_nonzero_subsecond_no_timestomp_annotation() {
    let cands = interpret::interpret_int(FILETIME_SUBSEC_NONZERO);
    let ft = cands
        .iter()
        .find(|c| c.format_id == "filetime")
        .expect("filetime candidate must be present");
    let joined = ft.assumptions.join(" ").to_lowercase();
    assert!(
        !joined.contains("manipulation"),
        "a filetime with non-zero sub-second field must NOT carry a manipulation note; got: {:?}",
        ft.assumptions
    );
}

#[test]
fn active_zero_subsecond_gets_no_filetime_manipulation_note() {
    // AD `active` shares the 100 ns-since-1601 encoding, but whole-second AD
    // values are common and legitimate — the SetFileTime signal must NOT leak
    // onto it (that would be a false positive / overstatement).
    let cands = interpret::interpret_int(FILETIME_WHOLE_SECOND);
    if let Some(active) = cands.iter().find(|c| c.format_id == "active") {
        let joined = active.assumptions.join(" ").to_lowercase();
        assert!(
            !joined.contains("setfiletime") && !joined.contains("manipulation"),
            "active (AD) must NOT carry the SetFileTime manipulation note; got: {:?}",
            active.assumptions
        );
    }
}

#[test]
fn dotnet_ticks_zero_subsecond_no_annotation() {
    // dotnet_ticks uses a different epoch (0001-01-01) and API context (.NET);
    // the SetFileTime-style signal does not apply. Verify no annotation leaks.
    const DOTNET_WHOLE_SECOND: i64 = 637_455_456_000_000_000;
    let cands = interpret::interpret_int(DOTNET_WHOLE_SECOND);
    if let Some(dn) = cands.iter().find(|c| c.format_id == "dotnet_ticks") {
        let joined = dn.assumptions.join(" ").to_lowercase();
        assert!(
            !joined.contains("setfiletime") && !joined.contains("manipulation"),
            "dotnet_ticks must NOT carry the SetFileTime manipulation note; got: {:?}",
            dn.assumptions
        );
    }
}
