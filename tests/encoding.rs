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
