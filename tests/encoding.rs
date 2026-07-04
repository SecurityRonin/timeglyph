//! Input-encoding handling: the same instant can be presented in different byte
//! encodings, and a packed format's ON-DISK byte order differs from a packed
//! integer. The hex path must decode packed formats (FAT) from their on-disk
//! layout so an analyst with raw bytes gets the right instant, not a silently
//! wrong one (see docs/concepts/input-conventions.md).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::{format, interpret, Encoded, PosixNs};

/// The instant 2020-01-01T00:00:00Z, via the unix decoder (unix seconds is
/// itself anchored to the time-decode oracle in tests/oracle.rs).
fn instant_2020() -> PosixNs {
    format("unix").unwrap().decode_int(1_577_836_800).unwrap()
}

#[test]
fn encode_float_matches_time_decode_vectors() {
    // Tier-1: ground-truth values are `time-decode --timestamp "2020-01-01
    // 00:00:00"` output (third-party authored), NOT a round-trip of our own
    // encoder. See docs/validation.md.
    let inst = instant_2020();
    for (id, expected) in [
        ("ole", 43831.0_f64),           // Windows OLE Automation Date
        ("sqlite_julian", 2_458_849.5), // Julian Date decimal
        ("excel1904", 42369.0),         // Microsoft Excel 1904 Date
        ("cocoa_float", 599_529_600.0), // Apple NSDate - Mac Absolute
    ] {
        let got = format(id).unwrap().encode_float(inst).unwrap();
        assert!(
            (got - expected).abs() < 1e-6,
            "{id}: encoded {got}, oracle says {expected}"
        );
    }
}

#[test]
fn encode_dispatches_float_vs_int() {
    let inst = instant_2020();
    assert_eq!(
        format("unix").unwrap().encode(inst).unwrap(),
        Encoded::Int(1_577_836_800)
    );
    assert_eq!(
        format("ole").unwrap().encode(inst).unwrap(),
        Encoded::Float(43831.0)
    );
    // Display: int prints bare, float prints its decimal value.
    assert_eq!(
        format("unix").unwrap().encode(inst).unwrap().to_string(),
        "1577836800"
    );
    assert_eq!(
        format("ole").unwrap().encode(inst).unwrap().to_string(),
        "43831"
    );
}

#[test]
fn encode_float_rejects_non_float_formats() {
    assert!(format("unix")
        .unwrap()
        .encode_float(instant_2020())
        .is_err());
}

#[test]
fn encode_embedded_timestamp_bits_match_time_decode() {
    // Tier-1: values are `time-decode --timestamp "2020-01-01 00:00:00"` output.
    // Embedded IDs carry the timestamp in the high bits and worker/sequence in
    // the low `shift` bits; time-decode fills some low bits with sample/invented
    // data (e.g. LinkedIn's value is odd), so we compare the TIMESTAMP bits
    // (value >> shift) — what an encoder is actually responsible for — between
    // timeglyph's encoder and the third-party encoder. Not a self round-trip.
    let inst = instant_2020();
    // (id, shift_bits — the spec constant carried in registry.rs, oracle value)
    let vectors: &[(&str, u32, i64)] = &[
        ("snowflake", 22, 1_212_161_512_043_446_272), // Twitter time
        ("discord", 22, 661_720_242_585_600_000),     // Discord time
        ("mastodon", 16, 103_405_112_524_800_000),    // Mastodon time
        ("linkedin", 22, 6_617_927_201_590_237_494),  // LinkedIn Activity time
        ("tiktok", 32, 6_776_757_454_425_292_800),    // TikTok time
    ];
    for &(id, shift, oracle) in vectors {
        let tg = format(id).unwrap().encode_int(inst).unwrap();
        assert_eq!(
            tg >> shift,
            oracle >> shift,
            "{id}: timeglyph timestamp bits {} vs oracle {}",
            tg >> shift,
            oracle >> shift
        );
    }
}

#[test]
fn encode_embedded_far_future_overflows_rather_than_wraps() {
    // A snowflake ID is 63-bit with a 22-bit shift, so the ms timestamp caps
    // near year 2080; a far-future instant must error, not silently wrap.
    let far = format("unix").unwrap().decode_int(7_258_118_400).unwrap(); // ~year 2200
    assert!(format("snowflake").unwrap().encode_int(far).is_err());
}

/// Parse time-decode's on-disk FAT/exFAT/MS-DOS string (a big-endian hex literal
/// standing in for the four on-disk little-endian bytes) into timeglyph's packed
/// integer `date << 16 | time`. Shared by the packed-encode oracle tests.
fn fat_ondisk_to_packed(be_hex: u32) -> i64 {
    let b = be_hex.to_be_bytes();
    let date = u16::from_le_bytes([b[0], b[1]]);
    let time = u16::from_le_bytes([b[2], b[3]]);
    (i64::from(date) << 16) | i64::from(time)
}

#[test]
fn encode_fat_matches_time_decode_vector() {
    // Tier-1: `time-decode --timestamp "2020-01-01 00:00:00"` prints
    // "FAT Date + Time: 21500000" — the on-disk little-endian wFatDate/wFatTime
    // bytes. timeglyph's fat encoder must produce the packed integer those bytes
    // represent (date << 16 | time). Value sourced from the third-party oracle.
    let inst = instant_2020();
    let want = fat_ondisk_to_packed(0x2150_0000);
    assert_eq!(format("fat").unwrap().encode_int(inst).unwrap(), want);
}

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
