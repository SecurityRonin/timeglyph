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
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.code().unwrap_or(-1),
    )
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
fn sentinel_value_signals_ambiguity_via_exit_code() {
    // value 0 decodes to every epoch (a sentinel) — pipelines must get a nonzero
    // "review needed" signal, not a confident 0.
    let (_out, code) = run(&["0"]);
    assert_eq!(code, 2, "sentinel value 0 should exit 2 (ambiguous)");
}
