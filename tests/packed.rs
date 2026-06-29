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

#[test]
fn moto_packed() {
    // oracle: --moto 3705040f1232 -> 2025-05-04 15:18:50 (UTC). 6 bytes, one
    // per field: year(+1970) month day hour minute second.
    assert_packed("moto", 0x3705_040F_1232, "2025-05-04T15:18:50");
}

#[test]
fn symantec_packed() {
    // oracle: --symantec 3704040f1232 -> 2025-05-04 15:18:50 (UTC). Like moto,
    // but the month byte is +1.
    assert_packed("symantec", 0x3704_040F_1232, "2025-05-04T15:18:50");
}

#[test]
fn dvr_packed() {
    // oracle: --dvr 3f06f000 -> 2015-12-03 15:00:00. 32-bit MSB-first:
    // year(6,+2000) month(4) day(5) hour(5) minute(6) second(6).
    assert_packed("dvr", 0x3F06_F000, "2015-12-03T15:00:00");
}

#[test]
fn sony_packed() {
    // oracle: --sony 65dd4bb89000001 -> 2023-05-01 19:37:45 (UTC). Sonyflake:
    // (id >> 24) counts 10ms units since the 2014-09-01 scheme epoch.
    assert_packed("sony", 0x065D_D4BB_8900_0001, "2023-05-01T19:37:45");
}

#[test]
fn ns40_packed() {
    // oracle: --ns40 07e905040f1232 -> 2025-05-04 15:18:50 (UTC). 7 bytes:
    // year(BE u16) month day hour minute second, each a raw byte value.
    assert_packed("ns40", 0x0007_E905_040F_1232, "2025-05-04T15:18:50");
}

#[test]
fn ns40le_packed() {
    // oracle: --ns40le e90705040f1232 -> 2025-05-04 15:18:50 (UTC). Like ns40
    // but the year u16 is little-endian.
    assert_packed("ns40le", 0x00E9_0705_040F_1232, "2025-05-04T15:18:50");
}

#[test]
fn logtime_packed() {
    // oracle: --logtime 343a0d17037b0000 -> 2023-03-23 13:58:52 (UTC). 8 bytes,
    // reversed field order: sec min hour day month year(+1900) + 2 fillers.
    assert_packed("logtime", 0x343A_0D17_037B_0000, "2023-03-23T13:58:52");
}

#[test]
fn semioctet_packed() {
    // oracle: --semioctet 525040518105 -> 2025-05-04 15:18:50. 12 digits, each
    // pair nibble-swapped, then YY(+2000) MM DD HH MM SS.
    assert_packed("semioctet", 525_040_518_105, "2025-05-04T15:18:50");
}

#[test]
fn gsm_packed() {
    // oracle: --gsm 52504051810500 -> 2025-05-04 15:18:50 (UTC). 7 bytes,
    // semi-octet (per-byte nibble swap → decimal), YY(+2000)..SS + tz byte.
    assert_packed("gsm", 0x0052_5040_5181_0500, "2025-05-04T15:18:50");
}

#[test]
fn nokiale_packed() {
    // oracle: --nokiale 5a0f9dd1 -> 2025-05-04 15:18:51 (UTC). 4-byte LE: bytes
    // reversed, then a two's-complement count of seconds before 2050.
    assert_packed("nokiale", 0x5A0F_9DD1, "2025-05-04T15:18:51");
}
