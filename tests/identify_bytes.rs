//! `identify_bytes`: the byte-native sweep (every width/endian/packed lane) that
//! `interpret_hex` wraps. The library entry point for hex-editor plugins and the
//! bounded carve — a caller with raw bytes in hand shouldn't have to hex-encode
//! them first. Faithfulness to the proven hex path is asserted by equivalence.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret::{identify_bytes, interpret_hex, Candidate};

fn shape(groups: &[(String, Vec<Candidate>)]) -> Vec<(String, Vec<&'static str>)> {
    groups
        .iter()
        .map(|(label, cs)| (label.clone(), cs.iter().map(|c| c.format_id).collect()))
        .collect()
}

#[test]
fn identify_bytes_decodes_a_raw_double_as_cocoa_float() {
    // 8 raw bytes = IEEE-754 double 768064730.064939 (LE) = cocoa_float 2025-05-04.
    let bytes = [0xec, 0x4f, 0x08, 0x6d, 0xde, 0xe3, 0xc6, 0x41];
    let groups = identify_bytes(&bytes);
    let found = groups.iter().flat_map(|(_, cs)| cs).any(|c| {
        c.format_id == "cocoa_float"
            && c.rendered
                .as_deref()
                .unwrap_or("")
                .starts_with("2025-05-04")
    });
    assert!(
        found,
        "identify_bytes should surface cocoa_float from the raw-double lane"
    );
}

#[test]
fn identify_bytes_decodes_an_8byte_le_filetime() {
    // 133908455300649390 (AD active/filetime, 2025-05-04) as little-endian bytes.
    let bytes = 133_908_455_300_649_390_u64.to_le_bytes();
    let groups = identify_bytes(&bytes);
    let found = groups.iter().flat_map(|(_, cs)| cs).any(|c| {
        c.format_id == "filetime"
            && c.rendered
                .as_deref()
                .unwrap_or("")
                .starts_with("2025-05-04")
    });
    assert!(
        found,
        "identify_bytes should surface filetime from an 8-byte LE integer"
    );
}

#[test]
fn identify_bytes_matches_interpret_hex_for_the_same_bytes() {
    // The hex path must become a thin wrapper: identical (label, format-ids) shape.
    let bytes = [0xec, 0x4f, 0x08, 0x6d, 0xde, 0xe3, 0xc6, 0x41];
    assert_eq!(
        shape(&identify_bytes(&bytes)),
        shape(&interpret_hex("ec4f086ddee3c641").unwrap())
    );
}
