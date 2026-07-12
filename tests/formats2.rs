//! Format wave 2 — additional forensic timestamp encodings decoded from their raw
//! fields/bytes. Each value→answer below is a SPEC worked example (tier-1): the
//! epoch, byte layout, and units are stated by the authoritative spec, so the
//! expected instant is derivable from the documented construction.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::compose::{cp56time2a, ext4_extra, iso9660, oracle_date, udf};

#[test]
fn oracle_7byte_date_decodes_the_documented_layout() {
    // Oracle DATE internal 7-byte: [century+100, year_of_century+100, month, day,
    // hour+1, minute+1, second+1]. 2020-01-01 00:00:00 = [120,120,1,1,1,1,1].
    let inst = oracle_date([120, 120, 1, 1, 1, 1, 1]).unwrap();
    assert_eq!(inst.to_rfc3339().unwrap(), "2020-01-01T00:00:00Z");
    // A time-of-day example: 2024-07-15 13:30:45 = century 20, year 24 → [120,124,...],
    // hour 14, minute 31, second 46 (each +1).
    let inst2 = oracle_date([120, 124, 7, 15, 14, 31, 46]).unwrap();
    assert_eq!(inst2.to_rfc3339().unwrap(), "2024-07-15T13:30:45Z");
}

#[test]
fn iso9660_7byte_recording_date_decodes() {
    // ECMA-119 §9.1.5: [years since 1900, month, day, hour, minute, second,
    // offset from GMT in 15-minute intervals (signed)]. 2020-01-01 00:00:00 GMT.
    let inst = iso9660([120, 1, 1, 0, 0, 0, 0]).unwrap();
    assert_eq!(inst.to_rfc3339().unwrap(), "2020-01-01T00:00:00Z");
    // A +2h offset (8 × 15 min) means the wall time is 2h ahead of UTC → the
    // instant is 2h earlier: 10:00 at +02:00 = 08:00Z.
    let inst2 = iso9660([120, 6, 15, 10, 0, 0, 8]).unwrap();
    assert_eq!(inst2.to_rfc3339().unwrap(), "2020-06-15T08:00:00Z");
}

#[test]
fn ext4_extra_extends_epoch_and_carries_nanoseconds() {
    // ext4: 32-bit seconds since 1970 + a 32-bit `extra` (low 2 bits extend the
    // epoch by ×2^32 s; high 30 bits are nanoseconds). extra=0 → the plain second.
    assert_eq!(
        ext4_extra(1_577_836_800, 0).to_rfc3339().unwrap(),
        "2020-01-01T00:00:00Z"
    );
    // 500 ms rides in extra>>2: extra = 500_000_000 << 2.
    assert!(ext4_extra(1_577_836_800, 500_000_000 << 2)
        .to_rfc3339()
        .unwrap()
        .starts_with("2020-01-01T00:00:00.5"));
}

#[test]
fn cp56time2a_iec60870_decodes() {
    // IEC 60870-5 CP56Time2a, 7 bytes: [ms LE u16, minute+flags, hour+flags,
    // day+dayOfWeek, month, year(+2000)]. 2020-01-01 00:00:00.000.
    let inst = cp56time2a([0, 0, 0, 0, 1, 1, 20]).unwrap();
    assert_eq!(inst.to_rfc3339().unwrap(), "2020-01-01T00:00:00Z");
    // 2024-07-15 13:30:45.500: ms = 45500 = 0xB1BC → [0xBC,0xB1]; the flag bits in
    // the minute/hour/day/year bytes must be masked off.
    let inst2 = cp56time2a([0xBC, 0xB1, 0x80 | 30, 0x80 | 13, 0xE0 | 15, 7, 24]).unwrap();
    assert!(inst2
        .to_rfc3339()
        .unwrap()
        .starts_with("2024-07-15T13:30:45.5"));
}

#[test]
fn udf_ecma167_timestamp_decodes() {
    // ECMA-167 §7.3, 12 bytes: [TypeAndTimezone LE u16, year LE i16, month, day,
    // hour, minute, second, centiseconds, hundreds-of-micros, micros].
    // 2020-01-01 00:00:00 UTC, tz 0: year 2020 = 0x07E4 → [0xE4,0x07].
    let inst = udf([0x00, 0x10, 0xE4, 0x07, 1, 1, 0, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(inst.to_rfc3339().unwrap(), "2020-01-01T00:00:00Z");
    // tz +120 min (0x078) means wall time is 2h ahead → instant 2h earlier.
    let inst2 = udf([0x78, 0x10, 0xE4, 0x07, 6, 15, 10, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(inst2.to_rfc3339().unwrap(), "2020-06-15T08:00:00Z");
}
