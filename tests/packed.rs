//! Packed bit-field timestamp anchors (HANDOFF §5a long tail), each cross-checked
//! against the MIT `time-decode` oracle on its own published example value. These
//! formats encode civil Y/M/D/H/M/S directly as bit fields (LOCAL/naive time, no
//! offset), so the rendered instant is naive — the `Format` carries `LocalNaive`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::format;

/// Decode `value` under packed format `id`; assert it renders starting `prefix`.
fn assert_packed(id: &str, value: i64, prefix: &str) {
    let f = format(id).unwrap();
    let inst = f.decode_int(value).unwrap();
    let rendered = inst.to_rfc3339().unwrap_or_default();
    assert!(
        rendered.starts_with(prefix),
        "{id}({value:#x}) = {rendered:?}, expected to start {prefix:?}"
    );
}

#[test]
fn exfat_packed() {
    // oracle: --exfat 5aa47a59 -> 2025-05-04 15:18:50. 32-bit MSB-first:
    // year(7,+1980) month(4) day(5) hour(5) min(6) sec*2(5).
    assert_packed("exfat", 0x5AA4_7A59, "2025-05-04T15:18:50");
}

#[test]
fn dttm_packed() {
    // oracle: --dttm 8768f513 -> 2018-08-30 20:19:00. 32-bit MSB-first:
    // dow(3) year(9,+1900) month(4) day(5) hour(5) min(6); no seconds.
    assert_packed("dttm", 0x8768_F513, "2018-08-30T20:19:00");
}

#[test]
fn bitdate_packed() {
    // oracle: --bitdate d223957e -> 2025-05-04 15:18. Samsung/LG: bytes are
    // byte-reversed, then MSB-first year(12) month(4) day(5) hour(5) min(6).
    assert_packed("bitdate", 0xD223_957E, "2025-05-04T15:18:00");
}

#[test]
fn bitdec_packed() {
    // oracle: --bitdec 2123703250 -> 2025-05-04 15:18. Decimal, bit-packed:
    // yr=v>>20, mon=(v>>16)&15, day=(v>>11)&31, hr=(v>>6)&31, min=v&63.
    assert_packed("bitdec", 2_123_703_250, "2025-05-04T15:18:00");
}

#[test]
fn bcd_packed() {
    // oracle: --bcd 250506232221 -> 2025-05-06 23:22:21. 12 decimal digits as
    // BCD pairs YY(+2000) MM DD HH MM SS.
    assert_packed("bcd", 250_506_232_221, "2025-05-06T23:22:21");
}
