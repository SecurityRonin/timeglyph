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
