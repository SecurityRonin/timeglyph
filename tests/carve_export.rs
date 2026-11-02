//! Carve hit exports: JSONL (one hit per line, pipeable) and ImHex bookmarks
//! (so a malware analyst's hex editor lights up at each timestamp offset).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::carve::{carve, to_imhex_bookmarks, to_jsonl};

fn planted() -> Vec<timeglyph::carve::CarveHit> {
    // 3 pad + 8-byte LE FILETIME (2025-05-04) + 3 pad.
    let ft = 133_908_455_300_649_390_u64.to_le_bytes();
    let mut buf = vec![0xAA_u8; 3];
    buf.extend_from_slice(&ft);
    buf.extend_from_slice(&[0xBB; 3]);
    let win = (
        946_684_800_i128 * 1_000_000_000,
        1_893_456_000_i128 * 1_000_000_000,
    );
    carve(&buf, 0.85, Some(win))
}

#[test]
fn jsonl_is_one_parseable_object_per_line_with_offset_and_format() {
    let jsonl = to_jsonl(&planted());
    assert!(!jsonl.is_empty());
    for line in jsonl.lines() {
        let v: serde_json::Value = serde_json::from_str(line).expect("each line is valid JSON");
        assert!(v.get("offset").is_some(), "carries offset");
        assert!(v.get("format").is_some(), "carries format");
        assert!(v.get("score").is_some(), "carries score");
    }
    assert!(
        jsonl
            .lines()
            .any(|l| l.contains("\"filetime\"") && l.contains("\"offset\":3")),
        "the planted filetime at offset 3 appears in the JSONL"
    );
}

#[test]
fn imhex_bookmarks_mark_the_offset_and_width() {
    let bm = to_imhex_bookmarks(&planted());
    let v: serde_json::Value = serde_json::from_str(&bm).expect("valid ImHex bookmark JSON");
    let arr = v["bookmarks"].as_array().expect("bookmarks array");
    assert!(
        arr.iter()
            .any(|b| b["region"]["address"] == 3 && b["region"]["size"] == 8),
        "an 8-byte bookmark at address 3 (the planted FILETIME)"
    );
}

#[test]
fn timesketch_jsonl_carries_the_required_import_fields() {
    let jsonl = timeglyph::carve::to_timesketch_jsonl(&planted());
    assert!(!jsonl.is_empty());
    for line in jsonl.lines() {
        let v: serde_json::Value = serde_json::from_str(line).expect("valid Timesketch JSONL");
        // Timesketch requires message + datetime + timestamp_desc.
        assert!(v.get("message").is_some(), "message");
        assert!(v.get("datetime").is_some(), "datetime (ISO8601)");
        assert!(v.get("timestamp_desc").is_some(), "timestamp_desc");
    }
    assert!(
        jsonl
            .lines()
            .any(|l| l.contains("2025-05-04") && l.contains("filetime")),
        "the planted filetime becomes a Timesketch event"
    );
}

#[test]
fn timesketch_export_skips_an_unresolved_wall_clock() {
    // Timesketch's importer parses a designator-less datetime in the HOST
    // timezone, so exporting a naive value would anchor it to whatever machine
    // ran the import. A timeline event needs a resolved instant.
    let bytes = [0x70_u8, 0x40, 0x21, 0x32, 0x52, 0x24, 0x23]; // GSM, UTC-resolved
    let fat = [0x00_u8, 0x00, 0x00, 0x00]; // placeholder, replaced below
    let _ = fat;
    let hits = carve(&bytes, 0.0, None);
    let jsonl = timeglyph::carve::to_timesketch_jsonl(&hits);
    // The GSM reading resolves to a UTC instant, so it IS exported.
    assert!(
        jsonl.contains("gsm"),
        "a resolved instant is exported: {jsonl}"
    );
    // A naive lane (FAT) must not appear.
    assert!(
        !jsonl.contains("\"fat\""),
        "an unresolved wall-clock is not a timeline event: {jsonl}"
    );
}
