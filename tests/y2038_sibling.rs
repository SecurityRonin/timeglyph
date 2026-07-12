//! Y2038 signed/unsigned sibling in the byte path. A 32-bit `time_t` is SIGNED:
//! after 2038-01-19 it exceeds 2^31 and, read as signed, wraps to a negative
//! value that displays as 1901–1969. So a 4-byte field with the high bit set is
//! ambiguous — a large UNSIGNED time (2038–2106) OR a NEGATIVE signed time
//! (pre-1970, a wrapped post-2038 value). Both are surfaced; the analyst decides.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret::identify_bytes;

fn lanes(bytes: &[u8]) -> Vec<String> {
    identify_bytes(bytes)
        .into_iter()
        .map(|(label, _)| label.to_lowercase())
        .collect()
}

#[test]
fn high_bit_set_4byte_value_surfaces_both_unsigned_and_signed() {
    // u32 = 0xF0000000 = 4_026_531_840 (unsigned); i32 = -268_435_456 (signed).
    let bytes = 0xF000_0000_u32.to_le_bytes();
    let labels = lanes(&bytes);
    assert!(
        labels.iter().any(|l| l.contains("u32 le")),
        "the unsigned lane is present: {labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|l| l.contains("i32") || l.contains("signed")),
        "the signed i32 sibling lane is present for a high-bit-set value: {labels:?}"
    );
}

#[test]
fn high_bit_clear_4byte_value_has_no_redundant_signed_lane() {
    // 0x10000000 = 268_435_456: i32 == u32 (both positive), so no separate lane.
    let bytes = 0x1000_0000_u32.to_le_bytes();
    let signed = lanes(&bytes)
        .iter()
        .filter(|l| l.contains("signed"))
        .count();
    assert_eq!(
        signed, 0,
        "no redundant signed lane when the high bit is clear (i32 == u32)"
    );
}
