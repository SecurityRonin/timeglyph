//! Differential validation against an INDEPENDENT third-party oracle.
//!
//! `time-decode` (Corey Forman / digitalsleuth, MIT) is a separate
//! implementation of the same forensic timestamp formats. Agreement between it
//! and `timeglyph` raises each anchor from tier-2 (ground truth derived from the
//! documented construction) to **tier-1** (an independent third party's tool
//! confirms the answer). See `docs/validation.md` for the full battery, tiers,
//! and provenance.
//!
//! Env-gated (fleet standard): the test SKIPS cleanly when `time-decode` is not
//! on `PATH`, so it never breaks a normal build. To run it:
//!
//! ```text
//! pip install time-decode
//! cargo test --features leap --test oracle
//! ```
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

/// True when the `time-decode` oracle is available on `PATH`.
fn oracle_available() -> bool {
    Command::new("time-decode")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Query the oracle: run `time-decode <flag> <value>` and return the normalized
/// `YYYY-MM-DD HH:MM:SS` it prints. The output line is `Label: <date> <tz>`, and
/// no format label contains `": "`, so splitting on the first `": "` isolates the
/// date; the civil part is its first 19 characters.
fn oracle(flag: &str, value: &str) -> Option<String> {
    let out = Command::new("time-decode")
        .arg(flag)
        .arg(value)
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

/// Normalize a timeglyph RFC-3339 rendering to `YYYY-MM-DD HH:MM:SS`.
fn civil(rfc3339: &str) -> String {
    rfc3339.replacen('T', " ", 1).chars().take(19).collect()
}

/// Assert the oracle agrees with an already-rendered timeglyph instant.
fn agree(label: &str, tg_rfc3339: &str, flag: &str, value: &str) {
    let want = civil(tg_rfc3339);
    let got = oracle(flag, value).unwrap_or_else(|| panic!("{label}: no oracle output"));
    assert_eq!(got, want, "{label}: oracle {got:?} vs timeglyph {want:?}");
}

fn render_int(id: &str, value: i64) -> String {
    timeglyph::format(id)
        .unwrap()
        .decode_int(value)
        .unwrap()
        .to_rfc3339()
        .unwrap()
}

fn render_float(id: &str, value: f64) -> String {
    timeglyph::format(id)
        .unwrap()
        .decode_float(value)
        .unwrap()
        .to_rfc3339()
        .unwrap()
}

#[test]
fn differential_battery_posix_family() {
    if !oracle_available() {
        eprintln!("skipping: time-decode oracle not on PATH (see docs/validation.md)");
        return;
    }
    // (timeglyph id, value) ↔ (oracle flag, oracle input). The oracle input may
    // differ in encoding from timeglyph's (documented in validation.md).
    agree(
        "unix",
        &render_int("unix", 1_577_836_800),
        "--unixsec",
        "1577836800",
    );
    agree(
        "unix_ms",
        &render_int("unix_ms", 1_577_836_800_000),
        "--unixmilli",
        "1577836800000",
    );
    agree(
        "unix_us",
        &render_int("unix_us", 1_577_836_800_000_000),
        "--prtime",
        "1577836800000000",
    );
    agree(
        "filetime",
        &render_int("filetime", 132_223_104_000_000_000),
        "--active",
        "132223104000000000",
    );
    agree(
        "webkit",
        &render_int("webkit", 13_222_310_400_000_000),
        "--chrome",
        "13222310400000000",
    );
    agree(
        "hfsplus",
        &render_int("hfsplus", 3_660_681_600),
        "--hfsdec",
        "3660681600",
    );
    agree(
        "hfsplus-max",
        &render_int("hfsplus", 4_294_967_295),
        "--hfsdec",
        "4294967295",
    );
    agree(
        "dotnet_ticks",
        &render_int("dotnet_ticks", 630_822_816_000_000_000),
        "--dotnet",
        "630822816000000000",
    );
    agree(
        "cocoa",
        &render_int("cocoa", 599_529_600),
        "--mac",
        "599529600.0",
    );
    agree(
        "sqlite_julian",
        &render_float("sqlite_julian", 2_451_545.0),
        "--juliandec",
        "2451545.0",
    );
    // Embedded-ID schemes (real ids; epoch + shift verified by the oracle).
    agree(
        "discord",
        &render_int("discord", 175_928_847_299_117_063),
        "--discord",
        "175928847299117063",
    );
    agree(
        "snowflake",
        &render_int("snowflake", 1_189_581_422_684_274_688),
        "--twitter",
        "1189581422684274688",
    );
}

#[cfg(feature = "leap")]
#[test]
fn differential_battery_leap_family() {
    use timeglyph::leap;
    if !oracle_available() {
        eprintln!("skipping: time-decode oracle not on PATH (see docs/validation.md)");
        return;
    }
    // GPS / NTP map directly; TAI64's label is 2^62 + (TAI seconds since 1970),
    // and the oracle's --tai takes those TAI seconds, so pass (label − 2^62).
    agree(
        "gps",
        &leap::from_gps_seconds(1_261_872_018.0).utc_rfc3339,
        "--gps",
        "1261872018",
    );
    agree(
        "ntp",
        &leap::from_ntp_seconds(3_786_825_600).unwrap().utc_rfc3339,
        "--ntp",
        "3786825600",
    );
    let label: u64 = 4_611_686_020_005_224_741;
    let tai_seconds = label - (1u64 << 62);
    agree(
        "tai64",
        &leap::from_tai64(label).unwrap().utc_rfc3339,
        "--tai",
        &tai_seconds.to_string(),
    );
}
