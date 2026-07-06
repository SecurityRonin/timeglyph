//! Untrusted-input size caps. The scan and hex paths ingest arbitrary text
//! (stdin, a lens accessibility buffer, a pasted blob); a pathological input
//! must bound the work rather than allocate unboundedly. Over-length hex fails
//! loud; over-long scan text is truncated to a documented cap.
#![allow(clippy::unwrap_used)]

use timeglyph::{interpret, scan, RenderZone};

#[test]
fn interpret_hex_rejects_oversized_input() {
    // A hex blob past the byte cap must error loudly, not build an unbounded
    // byte vec + per-window candidate lists.
    let huge = "ab".repeat(200_000); // ~200 KB decoded, well past the cap
    assert!(
        interpret::interpret_hex(&huge).is_err(),
        "over-cap hex must error"
    );
    // A normal small hex value still decodes.
    assert!(interpret::interpret_hex("0060947C58B2D501").is_ok());
}

#[test]
fn scan_bounds_pathological_input() {
    // A value placed beyond the scan byte cap is not scanned (input bounded);
    // a value within the cap still is.
    let mut text = " ".repeat(scan::MAX_SCAN_BYTES + 4096);
    text.push_str(" 1577836800 ");
    assert!(
        scan::inspect_text(&text, 5, &RenderZone::Utc).is_empty(),
        "a value beyond MAX_SCAN_BYTES must not be scanned"
    );
    assert!(
        !scan::inspect_text("ts 1577836800 end", 5, &RenderZone::Utc).is_empty(),
        "a value within the cap is still scanned"
    );
}
