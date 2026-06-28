//! Input-encoding handling: the same instant can be presented in different byte
//! encodings, and a packed format's ON-DISK byte order differs from a packed
//! integer. The hex path must decode packed formats (FAT) from their on-disk
//! layout so an analyst with raw bytes gets the right instant, not a silently
//! wrong one (see docs/concepts/input-conventions.md).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

#[test]
fn fat_on_disk_hex_decodes_to_fat() {
    // The FAT/DOS on-disk layout stores a date word then a time word, each
    // little-endian. time-decode's example `a45a597a` => 2025-05-04 15:18:50.
    let groups = interpret::interpret_hex("a45a597a").unwrap();
    assert!(
        groups
            .iter()
            .any(|(label, cands)| label.to_lowercase().contains("fat")
                && cands.iter().any(|c| c.format_id == "fat"
                    && c.rendered
                        .as_deref()
                        .unwrap_or("")
                        .starts_with("2025-05-04T15:18:50"))),
        "expected a FAT on-disk candidate from a45a597a: {groups:?}"
    );
}

#[test]
fn fat_hex_offers_both_word_orders() {
    // The same 4 bytes are ambiguous: the DOS packed convention is date-word then
    // time-word, but a FAT DIRECTORY entry stores time-word then date-word. Feeding
    // raw directory bytes under the wrong order silently swaps date and time, so
    // BOTH orders must be surfaced and clearly labelled (let the analyst choose).
    let groups = interpret::interpret_hex("a45a597a").unwrap();
    let date_time = groups
        .iter()
        .any(|(l, c)| l.contains("date|time") && c.iter().any(|x| x.format_id == "fat"));
    let time_date = groups
        .iter()
        .any(|(l, c)| l.contains("time|date") && c.iter().any(|x| x.format_id == "fat"));
    assert!(date_time, "missing date|time order: {groups:?}");
    assert!(time_date, "missing time|date (directory) order: {groups:?}");
}

#[test]
fn hex_notes_trailing_bytes() {
    // 6 bytes: the width decoders use the first 4/8; trailing bytes must be
    // surfaced, not silently dropped.
    let groups = interpret::interpret_hex("a45a597affff").unwrap();
    assert!(
        groups.iter().any(|(label, _)| label.contains("of 6")),
        "expected a 'first N of 6' note: {groups:?}"
    );
}

#[test]
fn hex_all_ones_u64_is_flagged_sentinel() {
    // 0xFFFFFFFFFFFFFFFF exceeds i64 so no linear candidate is produced; it must
    // still surface as an all-ones sentinel rather than vanish silently.
    let groups = interpret::interpret_hex("ffffffffffffffff").unwrap();
    assert!(
        groups
            .iter()
            .any(|(_, cands)| cands.iter().any(|c| c.sentinel)),
        "expected an all-ones sentinel candidate: {groups:?}"
    );
}
