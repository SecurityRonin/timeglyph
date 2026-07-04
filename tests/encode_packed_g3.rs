//! Packed-format ENCODERS, group 3: logtime, semioctet, gsm, nokiale, sqlserver.
//! Each `encode_<F>` is the exact inverse of its `decode_<F>` in src/registry.rs.
//!
//! Tier-1 validation (NOT a self round-trip): the oracle is the third-party
//! `time-decode` CLI. `encode_int` returns an INTEGER in `decode_<F>`'s
//! convention; the live tests convert that integer to the on-disk/hex/decimal
//! representation the matching `time-decode --<flag>` decoder expects, run that
//! decoder, and assert it reads the instant back as 2020-01-01 00:00:00.
//!
//! SQL Server has NO `time-decode` flag (the oracle does not implement it), so
//! its tier-1 check is the COMMITTED value derived from the documented spec
//! construction (int32 days since 1900-01-01 + uint32 1/300-second ticks); see
//! `sqlserver_committed_matches_spec_construction`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;
use timeglyph::{format, PosixNs};

/// 2020-01-01T00:00:00Z via the unix decoder (anchored to time-decode in
/// tests/oracle.rs). On-the-second so the sqlserver 1/300s tick has no rounding.
fn inst_2020() -> PosixNs {
    format("unix").unwrap().decode_int(1_577_836_800).unwrap()
}

/// True when the `time-decode` oracle is on PATH. Live tests early-return
/// (with an eprintln) when it is absent, like the oracle-gated tests elsewhere.
fn oracle_available() -> bool {
    Command::new("time-decode")
        .arg("--formats")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `time-decode --<flag> <repr>` and return its stdout.
fn time_decode(flag: &str, repr: &str) -> String {
    let out = Command::new("time-decode")
        .arg(flag)
        .arg(repr)
        .output()
        .expect("time-decode invocation failed");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The oracle prints e.g. "JET LogTime: 2020-01-01 00:00:00.000000 UTC"; assert
/// the decoded timestamp is 2020-01-01 00:00:00.
fn assert_decodes_to_2020(stdout: &str, flag: &str, repr: &str) {
    assert!(
        stdout.contains("2020-01-01 00:00:00"),
        "oracle {flag} {repr} did not decode to 2020-01-01 00:00:00: {stdout}"
    );
}

// ---- logtime ----------------------------------------------------------------

#[test]
fn logtime_live_roundtrips_through_time_decode() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let n = format("logtime").unwrap().encode_int(inst_2020()).unwrap();
    // decode_logtime reads a u64; time-decode --logtime wants the 8 on-disk
    // bytes as 16 hex chars (big-endian nibbles of that u64).
    let repr = format!("{:016x}", n as u64);
    assert_decodes_to_2020(&time_decode("--logtime", &repr), "--logtime", &repr);
}

#[test]
fn logtime_committed_matches_oracle_value() {
    // Tier-1: `time-decode --timestamp "2020-01-01 00:00:00"` prints JET LogTime
    // "0000000101780000". decode_logtime reads that as a big-endian u64, so the
    // packed integer is 0x0000000101780000 = 4_319_608_832.
    assert_eq!(
        format("logtime").unwrap().encode_int(inst_2020()).unwrap(),
        0x0000_0001_0178_0000
    );
}

#[test]
fn logtime_rejects_out_of_range() {
    // Pre-1900 has no room in the year+1900 byte field.
    let y1899 = format("unix").unwrap().decode_int(-2_240_524_800).unwrap(); // 1899-01-01
    assert!(format("logtime").unwrap().encode_int(y1899).is_err());
}

// ---- semioctet --------------------------------------------------------------

#[test]
fn semioctet_live_roundtrips_through_time_decode() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let n = format("semioctet")
        .unwrap()
        .encode_int(inst_2020())
        .unwrap();
    // decode_semioctet reads a 12-digit decimal integer; the oracle wants those
    // 12 digits verbatim (zero-padded).
    let repr = format!("{n:012}");
    assert_decodes_to_2020(&time_decode("--semioctet", &repr), "--semioctet", &repr);
}

#[test]
fn semioctet_committed_matches_oracle_value() {
    // Tier-1: `time-decode --timestamp` prints Semi-Octet decimal "021010000000".
    // decode_semioctet reads that 12-digit string as the integer 21_010_000_000.
    assert_eq!(
        format("semioctet")
            .unwrap()
            .encode_int(inst_2020())
            .unwrap(),
        21_010_000_000
    );
}

#[test]
fn semioctet_rejects_out_of_range() {
    // Year 1999 is below the YY+2000 field (would be negative).
    let y1999 = format("unix").unwrap().decode_int(915_148_800).unwrap(); // 1999-01-01
    assert!(format("semioctet").unwrap().encode_int(y1999).is_err());
}

// ---- gsm --------------------------------------------------------------------

#[test]
fn gsm_live_roundtrips_through_time_decode() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let n = format("gsm").unwrap().encode_int(inst_2020()).unwrap();
    // decode_gsm reads a 7-byte u64; time-decode --gsm wants 14 hex chars.
    let repr = format!("{:014x}", n as u64);
    assert_decodes_to_2020(&time_decode("--gsm", &repr), "--gsm", &repr);
}

#[test]
fn gsm_committed_matches_oracle_value() {
    // Tier-1: `time-decode --timestamp` prints GSM time "02101000000000" (7
    // bytes: nibble-swapped YY MM DD HH MM SS + a UTC tz byte 0x00). Read as a
    // big-endian u64 that is 0x02101000000000 = 580_610_858_942_464.
    assert_eq!(
        format("gsm").unwrap().encode_int(inst_2020()).unwrap(),
        0x0210_1000_0000_00
    );
}

#[test]
fn gsm_rejects_out_of_range() {
    // Year 1999 is below the YY+2000 field.
    let y1999 = format("unix").unwrap().decode_int(915_148_800).unwrap();
    assert!(format("gsm").unwrap().encode_int(y1999).is_err());
}

// ---- nokiale ----------------------------------------------------------------

#[test]
fn nokiale_live_roundtrips_through_time_decode() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let n = format("nokiale").unwrap().encode_int(inst_2020()).unwrap();
    // decode_nokiale reads a byte-reversed u32; time-decode --nokiale wants the
    // 4 on-disk bytes as 8 hex chars.
    let repr = format!("{:08x}", n as u32);
    assert_decodes_to_2020(&time_decode("--nokiale", &repr), "--nokiale", &repr);
}

#[test]
fn nokiale_committed_matches_oracle_value() {
    // Tier-1: `time-decode --timestamp` prints Nokia time LE "ff6a91c7".
    // decode_nokiale reads that as a u32 = 0xff6a91c7 = 4_285_174_215.
    assert_eq!(
        format("nokiale").unwrap().encode_int(inst_2020()).unwrap(),
        0xff6a_91c7
    );
}

#[test]
fn nokiale_rejects_out_of_range() {
    // After 2050 the seconds-before-2050 count goes negative and cannot pack.
    let y2100 = format("unix").unwrap().decode_int(4_102_444_800).unwrap(); // 2100-01-01
    assert!(format("nokiale").unwrap().encode_int(y2100).is_err());
}

// ---- sqlserver --------------------------------------------------------------

#[test]
fn sqlserver_committed_matches_spec_construction() {
    // Tier-1 by CONSTRUCTION (time-decode has no SQL Server flag). SQL Server
    // `datetime` = int32 days since 1900-01-01 + uint32 ticks of 1/300 s. For
    // 2020-01-01 00:00:00: days = 43829, ticks = 0. decode_sqlserver reads the
    // packed i64 as (days << 32) | ticks, so the expected value is
    // 43829 << 32 = 188_244_121_616_384. (Round-trip through decode_sqlserver is
    // exercised by the general decode tests; this pins the encoded integer to the
    // documented construction, not to our own decoder's output.)
    assert_eq!(
        format("sqlserver")
            .unwrap()
            .encode_int(inst_2020())
            .unwrap(),
        43_829_i64 << 32
    );
}

#[test]
fn sqlserver_rejects_out_of_range() {
    // An instant outside jiff's timestamp range yields no day arithmetic at all.
    assert!(format("sqlserver")
        .unwrap()
        .encode_int(PosixNs(i128::MAX))
        .is_err());
}
