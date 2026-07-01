//! Hostile-input hardening (from an adversarial code review). The cardinal sin
//! for a forensic tool is silently turning invalid input into a plausible date.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::{format, interpret};

#[test]
fn float_decode_rejects_non_finite_and_out_of_range() {
    let ole = format("ole").unwrap();
    assert!(ole.decode_float(f64::NAN).is_err(), "NaN must not decode");
    assert!(
        ole.decode_float(f64::INFINITY).is_err(),
        "inf must not decode"
    );
    assert!(
        ole.decode_float(1.0e30).is_err(),
        "absurd magnitude must not silently saturate to a date"
    );
    // a real value still works
    assert!(ole.decode_float(44197.0).unwrap().to_rfc3339().is_some());
}

#[test]
fn asn1_rejects_out_of_range_offset() {
    // +1260 has minute field 60 — not a valid offset; must NOT be silently
    // accepted (it would otherwise normalise to ~+13:00 and fabricate an instant).
    let cands = interpret::interpret_string("20200101000000+1260");
    assert!(
        !cands.iter().any(|c| c.format_id == "asn1_generalizedtime"),
        "invalid offset must not yield a GeneralizedTime reading: {cands:?}"
    );
    // a valid offset still parses
    let ok = interpret::interpret_string("20200101000000+0100");
    assert!(ok.iter().any(|c| c.format_id == "asn1_generalizedtime"));
}

#[test]
fn interpret_hex_never_panics_on_fuzz_crash_input() {
    // Regression: cargo-fuzz `interpret_hex` found this 21-byte input triggered
    // a panic (arithmetic overflow under debug-assertions). The no-panic
    // invariant requires Ok or Err — never a crash.
    let crash = "C80000000000000000000000000002626002626060";
    let _ = interpret::interpret_hex(crash); // must not panic
}

#[test]
fn interpret_string_never_panics_on_multibyte_tz_split() {
    // Regression: cargo-fuzz `interpret_string` found "Ť\0\u{8}2[" panicked in
    // split_tz — `split_at(len - 5)` used a byte index that landed inside the
    // 2-byte 'Ť'. Multi-byte input must return readings/none, never crash.
    let crash = std::str::from_utf8(&[197u8, 164, 0, 8, 50, 91]).unwrap();
    let _ = interpret::interpret_string(crash); // must not panic
                                                // a couple more multi-byte tails around the 5-byte tz-suffix boundary
    for s in ["ends in émoji 🕰", "＋０８００", "Ｚ", "x🕰4", "1577836800Ｚ"] {
        let _ = interpret::interpret_string(s);
    }
}
