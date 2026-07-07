//! 32-bit field-boundary annotation contract.
//!
//! A stored value near or past the signed 32-bit maximum (2^31 − 1 = 2147483647)
//! is forensically relevant for any 4-byte `LinearInt` field: it is near the
//! representable limit of a signed 32-bit integer (the canonical instance is the
//! Unix Y2038 boundary, but the fact is about the VALUE and the FIELD WIDTH, not
//! a calendar year — a 1904- or 2000-epoch 32-bit field has the same limit at a
//! different date). timeglyph annotates this as an **assumption**, framed
//! "consistent with", never a verdict.
//!
//! Scoped to 4-byte (`storage_bytes() == 4`) `LinearInt` formats only: an 8-byte
//! field (FILETIME, .NET ticks, ms/µs/ns counts) has no 32-bit boundary, so it
//! must never carry the note.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

/// The `unix` candidate for a value, or panic.
fn unix_assumptions(value: i64) -> Vec<String> {
    interpret::interpret_int(value)
        .into_iter()
        .find(|c| c.format_id == "unix")
        .unwrap_or_else(|| panic!("no unix candidate for {value}"))
        .assumptions
}

#[test]
fn value_approaching_signed_32bit_max_carries_boundary_note() {
    // 2_100_000_000 s ≈ 2036-07 — within ~2 years of 2^31 (2038-01-19).
    let joined = unix_assumptions(2_100_000_000).join(" ").to_lowercase();
    assert!(
        joined.contains("consistent with")
            && joined.contains("32-bit")
            && (joined.contains("limit")
                || joined.contains("maximum")
                || joined.contains("boundary")),
        "a value near 2^31 must carry a 'consistent with … 32-bit … limit' note; got: {joined:?}"
    );
    // Framed as a possibility, never a verdict.
    assert!(
        !joined.contains("will roll over") && !joined.contains("has rolled over"),
        "must not assert a verdict: {joined:?}"
    );
}

#[test]
fn value_past_signed_but_within_unsigned_32bit_is_flagged() {
    // 3_000_000_000 s: > 2^31 (2147483648) but < 2^32 (4294967296).
    let joined = unix_assumptions(3_000_000_000).join(" ").to_lowercase();
    assert!(
        joined.contains("32-bit")
            && (joined.contains("exceeds") || joined.contains("unsigned") || joined.contains("past")),
        "a value in (2^31, 2^32) must be flagged relative to the signed 32-bit range; got: {joined:?}"
    );
}

#[test]
fn ordinary_in_range_value_has_no_boundary_note() {
    // 2020-01-01: far below 2^31, no boundary concern.
    let joined = unix_assumptions(1_577_836_800).join(" ").to_lowercase();
    assert!(
        !joined.contains("32-bit"),
        "an ordinary value must carry no 32-bit boundary note; got: {joined:?}"
    );
}

#[test]
fn eight_byte_formats_never_carry_the_32bit_note() {
    // unix_ms is an 8-byte field; a 2020 unix_ms value has no 32-bit boundary.
    let ms = interpret::interpret_int(1_577_836_800_000)
        .into_iter()
        .find(|c| c.format_id == "unix_ms")
        .expect("unix_ms candidate");
    let joined = ms.assumptions.join(" ").to_lowercase();
    assert!(
        !joined.contains("32-bit"),
        "an 8-byte format must never carry a 32-bit boundary note; got: {joined:?}"
    );
}
