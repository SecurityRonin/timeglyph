//! Sentinel / "magic" value detection.
//!
//! Many timestamp fields use special values to mean "unset", "never", or "error"
//! rather than a real instant: a zero FILETIME, `accountExpires = 0`, all-ones
//! values. Decoding these to a plausible-looking date is the classic silent-wrong
//! forensic failure. The engine must flag them and rank them down — never hide
//! them, but never let them look authoritative.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

#[test]
fn zero_is_flagged_as_a_sentinel() {
    // value 0 decodes to each format's bare epoch — almost always an unset field.
    let cands = interpret::interpret_int(0);
    let cocoa = cands
        .iter()
        .find(|c| c.format_id == "cocoa")
        .expect("cocoa candidate"); // cocoa(0) = 2001-01-01, inside the window
    assert!(cocoa.sentinel, "value 0 must be flagged as a sentinel");
    assert!(
        cocoa
            .assumptions
            .iter()
            .any(|a| a.to_lowercase().contains("sentinel") || a.to_lowercase().contains("unset")),
        "sentinel reading must carry an explanatory assumption: {:?}",
        cocoa.assumptions
    );
}

#[test]
fn sentinel_does_not_outrank_a_real_value() {
    // cocoa(0) = 2001-01-01 is in-window and would otherwise score high; the
    // sentinel penalty must pull it below a genuine in-window reading.
    let zero = interpret::interpret_int(0);
    let cocoa_zero = zero.iter().find(|c| c.format_id == "cocoa").unwrap().score;
    let real = interpret::interpret_int(1_577_836_800);
    let unix_real = real.iter().find(|c| c.format_id == "unix").unwrap().score;
    assert!(
        unix_real > cocoa_zero,
        "real reading {unix_real} must outrank sentinel {cocoa_zero}"
    );
}

#[test]
fn real_value_is_not_a_sentinel() {
    let cands = interpret::interpret_int(1_577_836_800);
    let unix = cands.iter().find(|c| c.format_id == "unix").unwrap();
    assert!(
        !unix.sentinel,
        "a genuine value must not be flagged sentinel"
    );
}
