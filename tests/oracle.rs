//! Differential validation against an INDEPENDENT third-party oracle.
//!
//! `time-decode` (Corey Forman / digitalsleuth, MIT) is a separate
//! implementation of the same forensic timestamp formats. Every input below is
//! `time-decode`'s OWN published example value for that format (so the input is
//! authored by an independent third party, not chosen by us), and the expected
//! answer is `time-decode`'s output. timeglyph agreeing on the third party's
//! value AND answer is **tier-1** differential validation. See
//! `docs/validation.md` for the full battery, tiers, and provenance.
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

/// Assert the oracle agrees with an already-rendered timeglyph instant (to the
/// second; sub-second is compared separately by the unit-level anchors).
fn agree(label: &str, tg_rfc3339: &str, flag: &str, value: &str) {
    let want = civil(tg_rfc3339);
    let got = oracle(flag, value).unwrap_or_else(|| panic!("{label}: no oracle output"));
    assert_eq!(got, want, "{label}: oracle {got:?} vs timeglyph {want:?}");
}

/// Like [`agree`], for a float-input format: the `f64` is parsed from the same
/// string passed to the oracle, so there is one source of truth and no
/// literal-precision drift between the two implementations.
fn agree_float(label: &str, id: &str, flag: &str, value: &str) {
    let v: f64 = value.parse().unwrap();
    agree(label, &render_float(id, v), flag, value);
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
    agree(
        "unix",
        &render_int("unix", 1_746_371_930),
        "--unixsec",
        "1746371930",
    );
    agree(
        "unix_ms",
        &render_int("unix_ms", 1_746_371_930_064),
        "--unixmilli",
        "1746371930064",
    );
    agree(
        "unix_us",
        &render_int("unix_us", 1_746_371_930_064_939),
        "--prtime",
        "1746371930064939",
    );
    agree(
        "filetime",
        &render_int("filetime", 133_908_455_300_649_390),
        "--active",
        "133908455300649390",
    );
    agree(
        "webkit",
        &render_int("webkit", 13_390_845_530_064_940),
        "--chrome",
        "13390845530064940",
    );
    agree(
        "hfsplus",
        &render_int("hfsplus", 3_829_216_730),
        "--hfsdec",
        "3829216730",
    );
    // Apple TN1150's stated maximum HFS+ date — third-party answer + oracle.
    agree(
        "hfsplus-max",
        &render_int("hfsplus", 4_294_967_295),
        "--hfsdec",
        "4294967295",
    );
    agree(
        "dotnet_ticks",
        &render_int("dotnet_ticks", 638_819_687_300_649_472),
        "--dotnet",
        "638819687300649472",
    );
    agree_float("cocoa_float", "cocoa_float", "--mac", "768064730.064939");
    agree_float(
        "sqlite_julian",
        "sqlite_julian",
        "--juliandec",
        "2460800.1380787035",
    );
    agree_float("ole", "ole", "--oleauto", "45781.638079455312");
    agree(
        "discord",
        &render_int("discord", 1_102_608_904_745_127_937),
        "--discord",
        "1102608904745127937",
    );
    agree(
        "snowflake",
        &render_int("snowflake", 1_189_581_422_684_274_688),
        "--twitter",
        "1189581422684274688",
    );
    // FAT: time-decode reads the 4 on-disk bytes a4 5a 59 7a as
    // date = LE(a4,5a) = 0x5AA4, time = LE(59,7a) = 0x7A59; timeglyph takes those
    // two words packed into one int (date in the high word) = 0x5AA47A59. Same
    // instant, different input encoding (see validation.md "input conventions").
    agree("fat", &render_int("fat", 0x5AA4_7A59), "--fat", "a45a597a");
}

#[cfg(feature = "leap")]
#[test]
fn differential_battery_leap_family() {
    use timeglyph::leap;
    if !oracle_available() {
        eprintln!("skipping: time-decode oracle not on PATH (see docs/validation.md)");
        return;
    }
    agree(
        "gps",
        &leap::from_gps_seconds(1_430_407_111.0).utc_rfc3339,
        "--gps",
        "1430407111",
    );
    agree(
        "ntp",
        &leap::from_ntp_seconds(3_981_841_662).unwrap().utc_rfc3339,
        "--ntp",
        "3981841662.020607",
    );
    // TAI64's label is 2^62 + (TAI seconds since 1970); the oracle's --tai takes
    // those TAI seconds directly, so pass (label − 2^62).
    let tai_seconds: u64 = 1_599_755_800;
    agree(
        "tai64",
        &leap::from_tai64((1u64 << 62) + tai_seconds)
            .unwrap()
            .utc_rfc3339,
        "--tai",
        "1599755800",
    );
}
