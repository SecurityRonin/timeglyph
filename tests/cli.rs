//! CLI surface: subcommands (identify/decode/encode/hex/string/list) with a
//! back-compat bare-value shortcut, and pipeline-safe exit codes (0 ok,
//! 2 ambiguous/sentinel, 1 error). The binary is the Humble-Object shell; logic
//! is tested in the library, so these check wiring and exit codes only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

fn run(args: &[&str]) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_timeglyph"))
        .args(args)
        .output()
        .unwrap();
    // Combine stdout+stderr so tests can assert on warnings too.
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (combined, out.status.code().unwrap_or(-1))
}

#[test]
fn bare_value_back_compat_identifies() {
    let (out, _) = run(&["1577836800"]);
    assert!(out.contains("unix") && out.contains("2020-01-01"), "{out}");
}

#[test]
fn decode_subcommand() {
    let (out, code) = run(&["decode", "filetime", "132223104000000000"]);
    assert!(out.contains("2020-01-01"), "{out}");
    assert_eq!(code, 0);
}

#[test]
fn string_subcommand() {
    let (out, _) = run(&["string", "2020-01-01T00:00:00Z"]);
    assert!(out.contains("2020-01-01T00:00:00Z"), "{out}");
}

#[test]
fn hex_subcommand_decodes_fat_on_disk() {
    let (out, _) = run(&["hex", "a45a597a"]);
    assert!(out.contains("fat") && out.contains("2025-05-04"), "{out}");
}

#[test]
fn encode_subcommand() {
    let (out, code) = run(&["encode", "unix", "2020-01-01T00:00:00Z"]);
    assert!(out.contains("1577836800"), "{out}");
    assert_eq!(code, 0);
}

#[test]
fn list_subcommand() {
    let (out, _) = run(&["list"]);
    assert!(out.contains("filetime"), "{out}");
}

#[test]
fn decode_of_a_sentinel_warns_and_exits_ambiguous() {
    // `decode filetime 0` must not be a confident success — it is a sentinel.
    let (out, code) = run(&["decode", "filetime", "0"]);
    assert_eq!(code, 2, "decode of sentinel value 0 should exit 2: {out}");
    assert!(out.to_lowercase().contains("sentinel"), "{out}");
}

#[test]
fn hex_of_sentinel_bytes_exits_ambiguous() {
    // all-zero bytes decode to epoch sentinels under every width.
    let (_out, code) = run(&["hex", "00000000"]);
    assert_eq!(code, 2, "hex of all-zero bytes should exit 2 (ambiguous)");
}

#[test]
fn sentinel_value_signals_ambiguity_via_exit_code() {
    // value 0 decodes to every epoch (a sentinel) — pipelines must get a nonzero
    // "review needed" signal, not a confident 0.
    let (_out, code) = run(&["0"]);
    assert_eq!(code, 2, "sentinel value 0 should exit 2 (ambiguous)");
}

#[test]
fn identify_of_never_sentinel_surfaces_a_note() {
    // 0x7FFFFFFFFFFFFFFF ('never') renders out of civil range for every format,
    // so there are no candidates — but identify must still surface the sentinel
    // and exit 2 rather than print an unhelpful "no interpretation".
    let (out, code) = run(&["9223372036854775807"]);
    assert_eq!(code, 2, "{out}");
    assert!(
        out.to_lowercase().contains("sentinel") || out.to_lowercase().contains("never"),
        "{out}"
    );
}

#[test]
fn tz_flag_renders_identify_in_requested_zone() {
    // --tz changes the displayed offset; the unix reading of the 2020 instant
    // shows Tokyo local time (+09:00) instead of UTC `Z`.
    let (out, _) = run(&["1577836800", "--tz", "Asia/Tokyo"]);
    assert!(out.contains("2020-01-01T09:00:00+09:00"), "{out}");
}

#[test]
fn tz_flag_applies_to_decode() {
    let (out, code) = run(&["decode", "filetime", "132223104000000000", "--tz", "+08:00"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("2020-01-01T08:00:00+08:00"), "{out}");
}

#[test]
fn tz_flag_in_json_rewrites_rendered_field() {
    let (out, _) = run(&["1577836800", "--json", "--tz", "+08:00"]);
    assert!(out.contains("2020-01-01T08:00:00+08:00"), "{out}");
}

#[test]
fn unknown_tz_fails_loudly() {
    let (out, code) = run(&["1577836800", "--tz", "Not/AZone"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.to_lowercase().contains("zone"), "{out}");
}

#[test]
fn artifact_hint_surfaces_in_identify_output() {
    // A chrome hint must keep identify multi-candidate (never a verdict) and run
    // without error; the hint adjusts ranking, it does not filter.
    let (out, _) = run(&["13390845530064940", "--artifact", "chrome history"]);
    assert!(out.contains("webkit"), "{out}");
}

#[test]
fn csv_explicit_conversion_subcommand() {
    let p = std::env::temp_dir().join("tg_csv_explicit.csv");
    std::fs::write(&p, "id,created\n1,1577836800\n").unwrap();
    let (out, code) = run(&["csv", p.to_str().unwrap(), "--convert", "created:unix"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("created_unix"), "{out}");
    assert!(out.contains("2020-01-01T00:00:00Z"), "{out}");
}

#[test]
fn csv_auto_is_the_default() {
    let p = std::env::temp_dir().join("tg_csv_auto.csv");
    std::fs::write(&p, "name,ts\na,1577836800\n").unwrap();
    let (out, code) = run(&["csv", p.to_str().unwrap()]); // no flags → auto-detect
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("ts_unix") && out.contains("2020-01-01T00:00:00Z"),
        "{out}"
    );
}
