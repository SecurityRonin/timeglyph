//! Tier-1 differential validation of the group-1 packed ENCODERS against the
//! independent `time-decode` oracle (Corey Forman / digitalsleuth, MIT).
//!
//! For each format we encode the instant 2020-01-01T00:00:00Z with timeglyph,
//! convert timeglyph's integer to the representation `time-decode --<flag>`
//! expects, run that third-party decoder, and assert it decodes back to
//! 2020-01-01 00:00:00. If a SEPARATE implementation decodes our encoded value
//! to the right instant, the encoder is third-party validated — this is NOT a
//! self round-trip.
//!
//! Env-gated (fleet standard): the LIVE tests SKIP cleanly when `time-decode`
//! is not on `PATH`. The COMMITTED tests assert the exact integer derived from
//! `time-decode --timestamp "2020-01-01 00:00:00"` output, so they still run
//! (and regression-guard) without the oracle.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;
use timeglyph::{format, PosixNs};

/// True when the `time-decode` oracle is available on `PATH`.
fn oracle_available() -> bool {
    Command::new("time-decode")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The instant 2020-01-01T00:00:00Z, via the unix decoder (unix seconds is
/// itself anchored to the time-decode oracle in tests/oracle.rs).
fn inst_2020() -> PosixNs {
    format("unix").unwrap().decode_int(1_577_836_800).unwrap()
}

/// Run `time-decode <flag> <repr>` and return the civil `YYYY-MM-DD HH:MM:SS`
/// it prints (the output line is `Label: <date> <tz>`; splitting on the first
/// `": "` isolates the date, whose first 19 chars are the civil part).
fn oracle_decode(flag: &str, repr: &str) -> Option<String> {
    let out = Command::new("time-decode")
        .arg(flag)
        .arg(repr)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.contains(": "))?;
    let after = line.split_once(": ")?.1.trim();
    if after.len() < 19 {
        return None;
    }
    Some(after[..19].to_string())
}

// --- exfat -----------------------------------------------------------------
// time-decode --timestamp "2020-01-01 00:00:00" => exFAT time: 50210000 (8 hex
// chars, BE). 0x50210000 == 1_344_339_968, which is timeglyph's exFAT integer
// convention directly (decode_exfat reads the u32 as-is: p>>25 == 40 => 2020).

#[test]
fn encode_exfat_committed() {
    assert_eq!(
        format("exfat").unwrap().encode_int(inst_2020()).unwrap(),
        1_344_339_968
    );
}

#[test]
fn encode_exfat_oracle() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let v = format("exfat").unwrap().encode_int(inst_2020()).unwrap();
    let repr = format!("{:08x}", v as u32);
    let got = oracle_decode("--exfat", &repr).expect("exfat: no oracle output");
    assert_eq!(got, "2020-01-01 00:00:00", "exfat repr {repr}");
}

#[test]
fn encode_exfat_rejects_out_of_range() {
    // Year 1969 is below the exFAT 7-bit (+1980) field floor.
    let pre = format("unix").unwrap().decode_int(-31_536_000).unwrap();
    assert!(format("exfat").unwrap().encode_int(pre).is_err());
}

// --- dttm ------------------------------------------------------------------
// time-decode --timestamp "2020-01-01 00:00:00" => Microsoft DTTM: 67810800.
// The high 3 bits are dayOfWeek, which timeglyph's decoder IGNORES; timeglyph's
// encoder writes dayOfWeek=0, giving 0x07810800 == 125_896_704. time-decode
// decodes 07810800 to the same instant (dayOfWeek is display-only).

#[test]
fn encode_dttm_committed() {
    assert_eq!(
        format("dttm").unwrap().encode_int(inst_2020()).unwrap(),
        125_896_704
    );
}

#[test]
fn encode_dttm_oracle() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let v = format("dttm").unwrap().encode_int(inst_2020()).unwrap();
    let repr = format!("{:08x}", v as u32);
    let got = oracle_decode("--dttm", &repr).expect("dttm: no oracle output");
    assert_eq!(got, "2020-01-01 00:00:00", "dttm repr {repr}");
}

#[test]
fn encode_dttm_rejects_out_of_range() {
    assert!(format("dttm")
        .unwrap()
        .encode_int(PosixNs(i128::MAX))
        .is_err());
}

// --- bitdate ---------------------------------------------------------------
// time-decode --timestamp "2020-01-01 00:00:00" => BitDate: 0008417e (8 hex
// chars). timeglyph's decoder byte-swaps its input, so timeglyph's integer
// convention is 0x0008417e == 541_054 (decode_bitdate swaps to 0x7e410800).
// time-decode --bitdate 0008417e decodes to the same instant.

#[test]
fn encode_bitdate_committed() {
    assert_eq!(
        format("bitdate").unwrap().encode_int(inst_2020()).unwrap(),
        541_054
    );
}

#[test]
fn encode_bitdate_oracle() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let v = format("bitdate").unwrap().encode_int(inst_2020()).unwrap();
    let repr = format!("{:08x}", v as u32);
    let got = oracle_decode("--bitdate", &repr).expect("bitdate: no oracle output");
    assert_eq!(got, "2020-01-01 00:00:00", "bitdate repr {repr}");
}

#[test]
fn encode_bitdate_rejects_out_of_range() {
    assert!(format("bitdate")
        .unwrap()
        .encode_int(PosixNs(i128::MAX))
        .is_err());
}

// --- bitdec ----------------------------------------------------------------
// time-decode --bitdec 2118191104 => 2020-01-01 00:00:00. timeglyph's integer
// convention IS the decimal value directly (decode_bitdec unpacks it), so the
// expected integer is 2_118_191_104.

#[test]
fn encode_bitdec_committed() {
    assert_eq!(
        format("bitdec").unwrap().encode_int(inst_2020()).unwrap(),
        2_118_191_104
    );
}

#[test]
fn encode_bitdec_oracle() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let v = format("bitdec").unwrap().encode_int(inst_2020()).unwrap();
    let repr = v.to_string();
    let got = oracle_decode("--bitdec", &repr).expect("bitdec: no oracle output");
    assert_eq!(got, "2020-01-01 00:00:00", "bitdec repr {repr}");
}

#[test]
fn encode_bitdec_rejects_out_of_range() {
    // Pre-2000 year would pack a negative year<<20; below the format floor.
    assert!(format("bitdec")
        .unwrap()
        .encode_int(PosixNs(i128::MAX))
        .is_err());
}

// --- bcd -------------------------------------------------------------------
// time-decode --bcd 200101000000 => 2020-01-01 00:00:00. timeglyph reads the
// 12-digit decimal value (YY+2000 MM DD HH MM SS), so the expected integer is
// 200_101_000_000.

#[test]
fn encode_bcd_committed() {
    assert_eq!(
        format("bcd").unwrap().encode_int(inst_2020()).unwrap(),
        200_101_000_000
    );
}

#[test]
fn encode_bcd_oracle() {
    if !oracle_available() {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let v = format("bcd").unwrap().encode_int(inst_2020()).unwrap();
    let repr = v.to_string();
    let got = oracle_decode("--bcd", &repr).expect("bcd: no oracle output");
    assert_eq!(got, "2020-01-01 00:00:00", "bcd repr {repr}");
}

#[test]
fn encode_bcd_rejects_out_of_range() {
    // BCD stores a 2-digit YY (+2000); year 2100+ overflows the field.
    let y2100 = format("unix").unwrap().decode_int(4_102_444_800).unwrap();
    assert!(format("bcd").unwrap().encode_int(y2100).is_err());
}
