//! Bounded carve: slide `identify_bytes` over a blob and report scored hits per
//! offset — the validatable core of "find timestamps in raw bytes" (a config
//! blob, a hex selection, one record). A window + score threshold are the
//! false-positive controls; the caller bounds the input (not an image-wide
//! firehose). Ground truth is exact: we plant a known value at a known offset.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::carve::carve;

/// Nanosecond bounds for a plausibility window `[from, to)` in whole Unix years.
fn window(from_unix_secs: i64, to_unix_secs: i64) -> (i128, i128) {
    (
        i128::from(from_unix_secs) * 1_000_000_000,
        i128::from(to_unix_secs) * 1_000_000_000,
    )
}

#[test]
fn carve_finds_a_planted_filetime_at_its_offset() {
    // 3 pad bytes + an 8-byte little-endian FILETIME (2025-05-04) + 3 pad bytes.
    let ft = 133_908_455_300_649_390_u64.to_le_bytes();
    let mut buf = vec![0xAA_u8; 3];
    buf.extend_from_slice(&ft);
    buf.extend_from_slice(&[0xBB; 3]);

    // Window 2000-01-01 .. 2030-01-01; keep only strong readings.
    let win = window(946_684_800, 1_893_456_000);
    let hits = carve(&buf, 0.85, Some(win));

    let hit = hits
        .iter()
        .find(|h| h.offset == 3 && h.reading.format_id == "filetime")
        .expect("carve should find the FILETIME planted at offset 3");
    assert!(hit
        .reading
        .rendered
        .as_deref()
        .unwrap()
        .starts_with("2025-05-04"));
}

#[test]
fn carve_window_and_threshold_prune_noise() {
    // Random-ish bytes with no plausible 2020s timestamp; a tight window + high
    // threshold should yield few/no hits (the FP control actually controls).
    let noise = [0x11_u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    let win = window(1_577_836_800, 1_609_459_200); // 2020 only
    let strict = carve(&noise, 0.95, Some(win));
    assert!(
        strict.len() < carve(&noise, 0.0, None).len(),
        "a tight window + high threshold must prune relative to unfiltered carve"
    );
}

#[test]
fn carve_is_bounded_and_never_panics_on_empty_or_odd_input() {
    assert!(carve(&[], 0.5, None).is_empty());
    // A single byte: no 4/8-byte reading fits, must not panic.
    let _ = carve(&[0x42], 0.5, None);
}
