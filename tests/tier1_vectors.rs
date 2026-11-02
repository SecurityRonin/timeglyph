//! Tier-1 validation vectors — each pair below was authored by an INDEPENDENT
//! party (a published SMS tutorial, two independent SQL Server internals blogs, a
//! real exFAT volume read by the OS), not by a round-trip through the `time-decode`
//! tool. That independence is what lifts these three formats from tier-2 (real
//! oracle, our scenario) to tier-1 (an outside party authored BOTH the raw value
//! AND the expected instant). The decoder is unchanged; these anchor its output to
//! externally-published ground truth. See `docs/validation-tiers.tsv`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::format;

/// Decode `value` under packed format `id`; assert it renders starting `prefix`.
fn assert_vector(id: &str, value: i64, prefix: &str) {
    let inst = format(id).unwrap().decode_int(value).unwrap();
    let rendered = inst.to_rfc3339().unwrap_or_default();
    assert!(
        rendered.starts_with(prefix),
        "{id}({value:#x}) = {rendered:?}, expected to start {prefix:?}"
    );
}

#[test]
fn gsm_scts_developershome_example() {
    // GSM SMS TP-Service-Centre-Time-Stamp (7 semi-octets, nibble-swapped BCD +
    // quarter-hour tz), per 3GPP TS 23.040 §9.2.3.11. The developershome SMS
    // tutorial publishes the worked example: stored bytes 0x70402132522423 decode
    // to 2007-04-12 23:25:42 GMT+8. Independent of time-decode.
    // https://www.developershome.com/sms/cmgrCommand3.asp (§26.2.6.3)
    //
    // The value is LOCAL to the sending entity, and the tz octet 0x23 encodes
    // +32 quarter-hours = +8 h (TS 23.040 §9.2.3.11: "the first bit ... represents
    // the algebraic sign ... (0: positive, 1: negative)"; low nibble 3 = sign 0 +
    // tens 3, high nibble 2 = units 2). So the UTC instant is 23:25:42 - 8 h =
    // 2007-04-12T15:25:42Z. Reading the wall clock as UTC was an 8-hour error.
    assert_vector("gsm", 0x0070_4021_3252_2423, "2007-04-12T15:25:42");
}

#[test]
fn gsm_scts_negative_offset() {
    // A negative offset: tz octet 0x29 -> low nibble 0b1001 (sign bit set =
    // negative, tens 1), high nibble 2 (units 2) -> -12 quarter-hours = -3 h.
    // Wall clock 2007-04-12 23:25:42 at -03:00 is 2007-04-13T02:25:42Z.
    assert_vector("gsm", 0x0070_4021_3252_2429, "2007-04-13T02:25:42");
}

#[test]
fn gsm_rejects_a_malformed_timezone_octet() {
    // The tz octet is BCD: each digit must be 0-9. A units nibble of 0xA-0xF is
    // not a digit, so the octet is malformed and must be rejected rather than
    // read as a number (0xA0 would otherwise decode as 10 quarter-hours).
    let f = format("gsm").unwrap();
    assert!(
        f.decode_int(0x0070_4021_3252_24A0).is_err(),
        "units nibble 0xA is not a BCD digit"
    );
    assert!(
        f.decode_int(0x0070_4021_3252_24F0).is_err(),
        "units nibble 0xF is not a BCD digit"
    );
}

#[test]
fn gsm_scts_zero_offset_is_utc() {
    // tz octet 0x00 -> +0 quarter-hours: the wall clock IS UTC.
    assert_vector("gsm", 0x0070_4021_3252_2400, "2007-04-12T23:25:42");
}

#[test]
fn sqlserver_datetime_two_independent_blogs() {
    // SQL Server legacy `datetime` = int32 days since 1900-01-01 + uint32 ticks of
    // 1/300 s. Two INDEPENDENT authors publish byte↔datetime pairs (each ran the
    // engine's CAST, big-endian date-then-time form):
    //   bornsql.ca:            0x0000ABA500000000 -> 2020-04-22 00:00:00.000
    //   sqlserverscience.com:  0x0000A55F00A4CB80 -> 2015-11-29 10:00:00.000
    // https://bornsql.ca/blog/how-sql-server-stores-data-types-datetime-date-time-and-datetime2/
    // https://www.sqlserverscience.com/internals/how-sql-server-stores-datetime/
    assert_vector(
        "sqlserver",
        0x0000_ABA5_0000_0000u64 as i64,
        "2020-04-22T00:00:00",
    );
    assert_vector(
        "sqlserver",
        0x0000_A55F_00A4_CB80u64 as i64,
        "2015-11-29T10:00:00",
    );
}

#[test]
fn exfat_timestamp_real_macos_artifact() {
    // Real exFAT volume minted on macOS (`newfs_exfat`), a file touched to a known
    // time, then the raw 32-bit Timestamp field read from its directory entry — the
    // OS is the independent oracle. On-disk field 0x5303B55D decodes to the naive
    // 2021-08-03 22:42:58 (macOS stores UTC in the "local" Timestamp field and the
    // zone in the separate UtcOffset byte; the OS displayed 2021-08-04 14:42:58
    // local). The packed decoder reads the field verbatim. exFAT spec §7.4.8.
    // https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification
    assert_vector("exfat", 0x5303_B55D, "2021-08-03T22:42:58");
}
