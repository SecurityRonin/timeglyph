//! Composite (two-word) decode: timestamps split across two integer fields, as
//! stored in registry exports, IE cookies, and malware configs. No single-value
//! tool reconstructs these. Oracle: time-decode --filetimelohi.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

#[test]
fn filetime_hilo_reconstructs_the_64bit_filetime() {
    // Low 0x69050000 + High 0x01d5c036 = FILETIME 132223104000000000 = 2020-01-01.
    let inst = timeglyph::compose::filetime_hilo(0x6905_0000, 0x01d5_c036).unwrap();
    assert_eq!(
        inst.render(&timeglyph::RenderZone::Utc).unwrap(),
        "2020-01-01T00:00:00Z"
    );
}

#[test]
fn filetime_hilo_agrees_with_time_decode() {
    let ok = Command::new("time-decode")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let out = Command::new("time-decode")
        .args(["--filetimelohi", "69050000:01d5c036"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("2020-01-01 00:00:00"), "oracle: {text}");
    let inst = timeglyph::compose::filetime_hilo(0x6905_0000, 0x01d5_c036).unwrap();
    assert!(inst
        .render(&timeglyph::RenderZone::Utc)
        .unwrap()
        .starts_with("2020-01-01T00:00:00"));
}

#[test]
fn filetime_hilo_out_of_range_errs_not_panics() {
    // A pair whose reconstructed FILETIME exceeds i64 must error, never panic
    // (untrusted input robustness).
    assert!(timeglyph::compose::filetime_hilo(0xffff_ffff, 0xffff_ffff).is_err());
}

#[test]
fn unix_sec_nsec_reconstructs_a_timespec() {
    // struct timespec (ext4/BTRFS/ZFS stat, protobuf Timestamp, Java Instant):
    // (seconds, nanoseconds). 1577836800 s + 500000000 ns = 2020-01-01 00:00:00.5.
    let inst = timeglyph::compose::unix_sec_nsec(1_577_836_800, 500_000_000);
    assert!(inst
        .render(&timeglyph::RenderZone::Utc)
        .unwrap()
        .starts_with("2020-01-01T00:00:00.5"));
}

#[test]
fn relative_adds_a_duration_to_an_anchor() {
    // Boot-relative times (Android elapsedRealtime = ms since boot, mach
    // continuous time = ns since boot): the value is a DURATION, resolved against
    // an anchor. 2020-01-01T00:00:00Z + 3_600_000 ms = 2020-01-01T01:00:00Z.
    let anchor = timeglyph::compose::unix_sec_nsec(1_577_836_800, 0);
    let inst = timeglyph::compose::relative(anchor, 3_600_000, timeglyph::Unit::Millis);
    assert_eq!(
        inst.render(&timeglyph::RenderZone::Utc).unwrap(),
        "2020-01-01T01:00:00Z"
    );
}

#[cfg(feature = "leap")]
#[test]
fn gps_week_tow_resolves_leap_correct_utc() {
    // GPS week 2000, time-of-week 0 = 1_209_600_000 GPS seconds since 1980-01-06,
    // leap-corrected to 2018-05-05T23:59:42Z. Oracle: time-decode --gps. GNSS
    // receiver time (u-blox, NMEA, Berla iVe, drone logs) is natively week+TOW.
    let r = timeglyph::compose::gps_week_tow(2000, 0.0);
    assert!(r.utc_rfc3339.starts_with("2018-05-05T23:59:42"), "{r:?}");
}
