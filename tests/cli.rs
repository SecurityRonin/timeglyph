//! CLI surface: subcommands (identify/decode/encode/scan/list) plus the `--as`
//! interpretation selector (auto|int|hex|string) with a back-compat bare-value
//! shortcut, and pipeline-safe exit codes (0 ok, 2 ambiguous/sentinel, 1 error).
//! The binary is the Humble-Object shell; logic is tested in the library, so
//! these check wiring and exit codes only.
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
fn as_string_decodes_a_datetime() {
    // `--as string` forces the string-form interpreter and shows the reading.
    let (out, _) = run(&["--as", "string", "2020-01-01T00:00:00Z"]);
    assert!(out.contains("2020-01-01T00:00:00Z"), "{out}");
}

#[test]
fn as_hex_decodes_fat_on_disk() {
    let (out, _) = run(&["--as", "hex", "a45a597a"]);
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
    let (_out, code) = run(&["--as", "hex", "00000000"]);
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

#[test]
fn scan_finds_timestamps_and_respects_min_digits() {
    // scan sweeps arbitrary text and decodes every candidate.
    let (out, code) = run(&["scan", "created=1577836800 v7"]);
    assert!(
        out.contains("1577836800") && out.contains("2020-01-01"),
        "{out}"
    );
    assert_eq!(code, 0);
    // A higher digit floor drops the 10-digit run.
    let (out2, _) = run(&["scan", "--min-digits", "20", "1577836800"]);
    assert!(!out2.contains("2020-01-01"), "min-digits floor: {out2}");
}

#[test]
fn bare_hex_with_letters_decodes_as_hex() {
    // A bare value that is all hex digits with at least one a-f letter is raw
    // bytes, not an integer — it must route to the hex byte-layout decoder.
    let (out, _) = run(&["0060947C58B2D501"]);
    assert!(out.contains("byte layout"), "{out}");
}

#[test]
fn id_is_an_alias_for_identify() {
    // `id` is a visible alias for the `identify` subcommand.
    let (out, code) = run(&["id", "1577836800"]);
    assert!(
        out.contains("unix") && out.contains("2020-01-01T00:00:00Z"),
        "{out}"
    );
    assert_eq!(code, 0);
}

#[test]
fn bare_datetime_string_decodes() {
    // A bare ASN.1 GeneralizedTime string must be parsed as a datetime string.
    let (out, _) = run(&["20200101000000Z"]);
    assert!(
        out.contains("asn1_generalizedtime") && out.contains("2020-01-01T00:00:00Z"),
        "{out}"
    );
}

#[test]
fn bare_iso_string_decodes() {
    // A bare ISO 8601 / RFC 3339 string must be parsed as a datetime string.
    let (out, _) = run(&["2020-01-01T00:00:00Z"]);
    assert!(out.contains("iso8601"), "{out}");
}

#[test]
fn bare_integer_still_identifies() {
    // A pure integer keeps identifying across numeric formats (back-compat).
    let (out, _) = run(&["1577836800"]);
    assert!(
        out.contains("unix") && out.contains("2020-01-01T00:00:00Z"),
        "{out}"
    );
}

#[test]
fn bare_all_digit_merges_int_and_string() {
    // An all-digit value that is a valid integer AND a valid 14-digit ASN.1
    // GeneralizedTime must merge both families' readings.
    let (out, _) = run(&["20200101000000"]);
    assert!(
        out.contains("iostime"),
        "expected an integer reading: {out}"
    );
    assert!(
        out.contains("asn1"),
        "expected an ASN.1 string reading: {out}"
    );
}

#[test]
fn as_string_on_a_bare_integer_fails() {
    // Forcing the string family on a bare integer (no self-describing form) must
    // fail loudly (exit 1), not silently fall through to the integer reading.
    let (_out, code) = run(&["--as", "string", "1577836800"]);
    assert_eq!(
        code, 1,
        "forcing string on a non-string value should exit 1"
    );
}

#[test]
fn as_int_on_a_non_integer_fails() {
    // Forcing the integer family on a value that is not an i64 (has letters) must
    // fail (exit 1) — no candidates, nothing to identify.
    let (_out, code) = run(&["--as", "int", "20200101000000Z"]);
    assert_eq!(code, 1, "forcing int on a non-integer should exit 1");
}

#[test]
fn as_hex_forces_hex_on_a_decimal_value() {
    // `--as hex` forces the hex byte-layout decoder even for a pure-decimal value.
    let (out, _) = run(&["--as", "hex", "1577836800"]);
    assert!(out.contains("byte layout"), "{out}");
}

#[test]
fn bare_0x_prefixed_value_decodes_as_hex() {
    // A `0x`-prefixed value is unambiguously raw hex bytes → hex byte-layout path.
    let (out, _) = run(&["0x0060947C58B2D501"]);
    assert!(out.contains("byte layout"), "{out}");
}

#[test]
fn removed_hex_subcommand_is_unknown() {
    // The `hex` subcommand was removed in favor of `--as hex`; invoking it is now
    // an unrecognized subcommand error.
    let (out, code) = run(&["hex", "00"]);
    assert_ne!(code, 0, "removed `hex` subcommand should error: {out}");
}

#[test]
fn removed_string_subcommand_is_unknown() {
    // The `string` subcommand was removed in favor of `--as string`.
    let (out, code) = run(&["string", "2020-01-01T00:00:00Z"]);
    assert_ne!(code, 0, "removed `string` subcommand should error: {out}");
}
