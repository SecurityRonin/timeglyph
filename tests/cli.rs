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
fn bare_fractional_float_identifies_cocoa_float() {
    // WhatsApp-iOS ZMESSAGEDATE is CFAbsoluteTime (a double). The auto path must
    // ingest the fraction and rank cocoa_float, keeping sub-second precision.
    let (out, code) = run(&["606940977.71577"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("cocoa_float"), "{out}");
    assert!(
        out.contains("2020-03-26T18:42:57.7"),
        "sub-second lost: {out}"
    );
}

#[test]
fn bare_integer_stays_integer_only() {
    // Regression: a pure integer keeps the integer decoders (cocoa), never the
    // float-strategy cocoa_float — the float path triggers only for a fraction.
    // (The exit code is 2 here: an unqualified integer is genuinely ambiguous.)
    let (out, _) = run(&["606940977"]);
    assert!(out.contains("cocoa"), "{out}");
    assert!(
        !out.contains("cocoa_float"),
        "float format leaked onto int path: {out}"
    );
}

#[test]
fn fractional_float_rejected_under_as_int() {
    // `--as int` means integer epochs only; a fractional literal must error.
    let (out, code) = run(&["--as", "int", "606940977.71577"]);
    assert_ne!(code, 0, "{out}");
}

#[test]
fn identify_local_naive_is_not_zone_shifted() {
    // A LocalNaive reading (exFAT/FAT wall-clock) must NOT be zone-shifted or
    // offset-stamped under --tz: the stored wall-clock (05:57:34) stands, same
    // as the scan/lens path. Bug was: the CLI rendered it 00:57:34-05:00.
    let (out, _) = run(&["606940977", "--tz", "America/New_York"]);
    assert!(
        out.contains("1998-01-13T05:57:34"),
        "local-naive wall-clock must be preserved: {out}"
    );
    assert!(
        !out.contains("1998-01-13T00:57:34"),
        "local-naive must not be zone-shifted: {out}"
    );
}

#[test]
fn decode_local_naive_is_not_zone_shifted() {
    // Same rule on the explicit decode path.
    let (out, _) = run(&["decode", "exfat", "606940977", "--tz", "America/New_York"]);
    assert!(out.contains("1998-01-13T05:57:34"), "{out}");
    assert!(
        !out.contains("00:57:34"),
        "must not zone-shift local-naive: {out}"
    );
}

#[test]
fn hex_input_exit_code_reflects_ambiguity() {
    // Byte-layout interpretation is inherently ambiguous (multiple layouts x
    // formats). The exit code must not claim an unambiguous OK when several
    // readings tie — it must apply the same ambiguity_code as identify.
    let (out, code) = run(&["--as", "hex", "0060947C58B2D501"]);
    assert!(!out.is_empty(), "{out}");
    assert_eq!(
        code, 2,
        "hex with tied top readings must be EXIT_AMBIGUOUS, not OK: {out}"
    );
}

#[test]
fn scan_json_emits_jsonl_with_schema_version() {
    // scan --json must emit one JSON object per found value (JSONL, streamable),
    // each carrying schema_version + the ranked readings — the pipeline path.
    let (out, _) = run(&["scan", "ts 1577836800 end", "--json"]);
    let line = out
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .expect("a JSON object line");
    let v: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(v["schema_version"], 1, "{line}");
    assert_eq!(v["number"], "1577836800", "{line}");
    let readings = v["readings"].as_array().unwrap();
    assert!(readings.iter().any(|r| r["format_id"] == "unix"), "{line}");
}

#[test]
fn encode_all_emits_per_format_hex_needles() {
    // `encode all <dt>` inverts the tool: for a known time, emit every format's
    // encoded value + on-disk hex bytes (LE/BE) — disk-search "needles".
    let (out, code) = run(&["encode", "all", "2020-01-01T00:00:00Z"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("unix"), "must list formats: {out}");
    // unix seconds 1577836800 = 0x5E0BE100 → LE 00e10b5e / BE 5e0be100 (4-byte).
    let lo = out.to_lowercase();
    assert!(
        lo.contains("00e10b5e") && lo.contains("5e0be100"),
        "must show LE + BE on-disk bytes: {out}"
    );
}

#[test]
fn scan_shows_all_readings_by_default() {
    // Show-all principle: scan must not filter by likelihood — every reading is
    // shown (out-of-window included), ranked; likelihood is information, not a
    // gate. The old default capped at 4 in-window readings; now the full set
    // shows and `--top` is the opt-in brevity knob.
    let (all_out, _) = run(&["scan", "v 4294967296 w"]);
    let n_all = all_out.lines().filter(|l| l.starts_with("    ")).count();
    assert!(
        n_all > 4,
        "default scan must show all readings (not cap at 4): got {n_all}"
    );
    let (top_out, _) = run(&["scan", "v 4294967296 w", "--top", "3"]);
    let n_top = top_out.lines().filter(|l| l.starts_with("    ")).count();
    assert!(
        n_top <= 3,
        "--top 3 must cap readings per value: got {n_top}"
    );
}

fn reading_lines(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|l| l.trim_start().starts_with('['))
        .collect()
}

fn score_of(line: &str) -> f64 {
    line.trim()
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn top_flag_limits_reading_count() {
    let (all, _) = run(&["1577836800"]);
    let (top3, _) = run(&["1577836800", "--top", "3"]);
    assert!(reading_lines(&all).len() > 3, "baseline: {all}");
    assert_eq!(reading_lines(&top3).len(), 3, "--top 3: {top3}");
}

#[test]
fn min_score_flag_filters_below_threshold() {
    let (out, _) = run(&["1577836800", "--min-score", "0.9"]);
    let lines = reading_lines(&out);
    assert!(!lines.is_empty(), "some readings should survive: {out}");
    for l in &lines {
        assert!(score_of(l) >= 0.9, "kept a below-threshold reading: {l}");
    }
}

#[test]
fn ambiguity_gap_flag_widens_the_tie_band() {
    // 1577836800: top two are 1.00 (unix) vs 0.86 (iostime), gap 0.14 — OK by
    // default (exact-tie only). The gap widened from the pre-epoch_distance 0.03:
    // the runner-up used to hug the iostime 2001 epoch and score ~0.97; the
    // MAGNITUDE/RECENCY prior now demotes that epoch-hugging reading.
    let (_, code_default) = run(&["1577836800"]);
    assert_eq!(code_default, 0, "default gap: unique top is unambiguous");
    // ...but AMBIGUOUS once the gap band is widened past 0.14. Require real
    // readings in the output so a clap usage error (also exit 2) can't pose as
    // ambiguity.
    let (out_wide, code_wide) = run(&["1577836800", "--ambiguity-gap", "0.20"]);
    assert!(
        out_wide.contains("unix"),
        "must print readings, not a clap error: {out_wide}"
    );
    assert_eq!(
        code_wide, 2,
        "near-tie within the gap band must be ambiguous"
    );
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
fn as_on_a_non_identify_command_errors() {
    // --as only affects identify; scan is always auto, so passing --as there is a
    // misuse that must fail loudly rather than be silently ignored.
    let (out, code) = run(&["--as", "hex", "scan", "hello 1577836800"]);
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("--as applies only to"), "{out}");
}

#[test]
fn artifact_on_a_non_identify_command_errors() {
    let (out, code) = run(&["decode", "unix", "1577836800", "--artifact", "ntfs"]);
    assert_ne!(code, 0, "{out}");
    assert!(out.contains("--artifact applies only to"), "{out}");
}

#[test]
fn global_flag_works_before_a_subcommand() {
    // The placement trap is gone: a global (--tz) before the subcommand now works,
    // not just after it.
    let (out, code) = run(&["--tz", "UTC", "decode", "unix", "1577836800"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("2020-01-01T00:00:00Z"), "{out}");
}

#[test]
fn removed_string_subcommand_is_unknown() {
    // The `string` subcommand was removed in favor of `--as string`.
    let (out, code) = run(&["string", "2020-01-01T00:00:00Z"]);
    assert_ne!(code, 0, "removed `string` subcommand should error: {out}");
}

#[test]
fn provenance_envelope_wraps_json_with_engine_and_digest() {
    let (out, _) = run(&["1577836800", "--json", "--provenance"]);
    let v: serde_json::Value = serde_json::from_str(out.trim()).expect("valid JSON envelope");
    assert_eq!(v["engine"], "timeglyph");
    assert!(
        v["engine_version"].as_str().is_some_and(|s| !s.is_empty()),
        "{out}"
    );
    assert_eq!(v["registry_digest"].as_str().unwrap().len(), 16);
    assert_eq!(v["input"], "1577836800");
    let readings = v["readings"].as_array().expect("readings array");
    assert!(readings.iter().any(|r| r["format_id"] == "unix"), "{out}");
    // Each reading carries its spec citation (traceable provenance).
    assert!(readings[0]["citation"].as_str().is_some(), "{out}");
}

#[test]
fn decode_filetime_hilo_composite() {
    // Two hex halves, low:high — the way FILETIME appears split in .reg exports.
    let (out, code) = run(&["decode", "filetime_hilo", "69050000:01d5c036"]);
    assert!(out.contains("2020-01-01"), "{out}");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn decode_unix_sec_nsec_composite() {
    let (out, code) = run(&["decode", "unix_sec_nsec", "1577836800:500000000"]);
    assert!(out.contains("2020-01-01T00:00:00.5"), "{out}");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn decode_elapsed_realtime_with_anchor() {
    // Relative decode: "<ticks>@<anchor>" — 3_600_000 ms after 2020-01-01T00:00Z.
    let (out, code) = run(&["decode", "elapsed_realtime", "3600000@2020-01-01T00:00:00Z"]);
    assert!(out.contains("2020-01-01T01:00:00"), "{out}");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn decode_gps_week_tow_composite() {
    // "week:tow" -> leap-correct UTC.
    let (out, code) = run(&["decode", "gps_week_tow", "2000:0"]);
    assert!(out.contains("2018-05-05T23:59:42"), "{out}");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn decode_syslog_infers_year() {
    let (out, code) = run(&["decode", "syslog", "Jan 12 06:30:00@2026-03-01T00:00:00Z"]);
    assert!(out.contains("2026-01-12T06:30:00"), "{out}");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn decode_vmsd_composite() {
    let (out, code) = run(&["decode", "vmsd", "367368,-1040564224"]);
    assert!(out.contains("2020-01-01T00:00:00"), "{out}");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn carve_subcommand_finds_a_planted_filetime_as_jsonl() {
    // buffer = AAAAAA + 8-byte LE FILETIME(2025-05-04) + BBBBBB; window 2000..2030.
    let (out, code) = run(&[
        "carve",
        "aaaaaaaed19dd607bddb01bbbbbb",
        "--json",
        "--from",
        "2000",
        "--to",
        "2030",
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("\"filetime\"") && out.contains("\"offset\":3"),
        "carve JSONL should report the filetime planted at offset 3; got: {out}"
    );
}

#[test]
fn explain_subcommand_prints_a_spec_card() {
    let (out, code) = run(&["explain", "filetime"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("1601") && out.to_lowercase().contains("epoch"),
        "explain should print the filetime spec card (epoch 1601); got: {out}"
    );
}

#[test]
fn explain_unknown_format_errors() {
    let (_out, code) = run(&["explain", "no-such-format"]);
    assert_eq!(code, 1, "an unknown format id must exit 1");
}

#[test]
fn localnaive_under_a_named_zone_flags_the_dst_fold() {
    // FAT 1399262144 = 2021-11-07 01:30:00 (naive) — a DST fall-back fold in NY,
    // where 01:30 maps to two instants: 05:30Z (EDT) and 06:30Z (EST).
    let (out, code) = run(&[
        "identify",
        "--json",
        "1399262144",
        "--tz",
        "America/New_York",
    ]);
    // The value reads across several formats, so exit 2 (ambiguous) is expected
    // and fine — the fold note is what this test checks.
    assert!(code == 0 || code == 2, "unexpected exit {code}: {out}");
    assert!(
        out.contains("fall-back fold")
            && out.contains("2021-11-07T05:30:00Z")
            && out.contains("2021-11-07T06:30:00Z"),
        "the fat reading flags the DST fold with both instants: {out}"
    );
}

#[test]
fn decode_wave2_byte_formats_via_cli() {
    // Oracle DATE [120,120,1,1,1,1,1] = 2020-01-01 00:00:00.
    let (out, code) = run(&["decode", "oracle_date", "78780101010101"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("2020-01-01T00:00:00"), "oracle_date: {out}");
    // ISO 9660 [120,1,1,0,0,0,0].
    let (out, _) = run(&["decode", "iso9660", "78010100000000"]);
    assert!(out.contains("2020-01-01T00:00:00"), "iso9660: {out}");
    // CP56Time2a [0,0,0,0,1,1,20].
    let (out, _) = run(&["decode", "cp56time2a", "00000000010114"]);
    assert!(out.contains("2020-01-01T00:00:00"), "cp56time2a: {out}");
    // UDF [0x00,0x10,0xE4,0x07,1,1,0,0,0,0,0,0].
    let (out, _) = run(&["decode", "udf", "0010e4070101000000000000"]);
    assert!(out.contains("2020-01-01T00:00:00"), "udf: {out}");
}

#[test]
fn decode_ext4_extra_via_cli() {
    let (out, code) = run(&["decode", "ext4_extra", "1577836800,0"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("2020-01-01T00:00:00"), "ext4_extra: {out}");
}

// --- cal subcommand -----------------------------------------------------------

#[test]
fn cal_month_renders_grid() {
    let (out, code) = run(&["cal", "2026-07"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("July 2026"), "{out}");
    assert!(out.contains("W27"), "{out}");
}

#[test]
fn cal_year_renders_twelve_months() {
    let (out, code) = run(&["cal", "2026"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("January 2026") && out.contains("December 2026"), "{out}");
}

#[test]
fn cal_json_is_machine_readable() {
    let (out, code) = run(&["cal", "2026-07", "--json"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"jdn\"") && out.contains("\"iso_week\""), "{out}");
}

#[test]
fn cal_bad_when_errors_with_the_value() {
    let (out, code) = run(&["cal", "notadate"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("notadate"), "{out}");
}

#[test]
fn cal_no_args_is_current_month() {
    let (out, code) = run(&["cal"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Mo") && out.contains("Su"), "{out}");
}
