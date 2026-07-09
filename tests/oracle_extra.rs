//! Additional independent oracles beyond `time-decode` + `unfurl`: CPython
//! `datetime`, `sqlite3`, and GNU `date` — each a separate implementation
//! cross-checking timeglyph's decode. Notably `sqlite3`'s `julianday()` is *the*
//! reference for the `sqlite_julian` format. Env-gated: each test skips cleanly
//! when its tool is absent (like the time-decode battery), so CI without the
//! tools still passes.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

/// timeglyph's civil rendering (`YYYY-MM-DDTHH:MM:SS`, UTC) of an integer decode.
fn tg_int(format_id: &str, value: i64) -> String {
    let inst = timeglyph::format(format_id)
        .unwrap()
        .decode_int(value)
        .unwrap();
    inst.render(&timeglyph::RenderZone::Utc).unwrap()[..19].to_string()
}

/// Same, for a float-strategy format (`decode_float`).
fn tg_float(format_id: &str, value: f64) -> String {
    let inst = timeglyph::format(format_id)
        .unwrap()
        .decode_float(value)
        .unwrap();
    inst.render(&timeglyph::RenderZone::Utc).unwrap()[..19].to_string()
}

/// Run a tool, returning trimmed stdout, or `None` if it's absent / errored
/// (env-gate: a missing oracle skips its test rather than failing the build).
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

const UNIX_V: i64 = 1_577_836_800; // 2020-01-01T00:00:00Z

#[test]
fn cpython_datetime_agrees_on_unix() {
    let code = format!(
        "import datetime;print(datetime.datetime.fromtimestamp({UNIX_V},datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%S'))"
    );
    let Some(got) = run("python3", &["-c", &code]) else {
        eprintln!("python3 absent — skipping");
        return;
    };
    assert_eq!(got, tg_int("unix", UNIX_V), "CPython vs timeglyph unix");
}

#[test]
fn sqlite3_agrees_on_unix() {
    let q = format!("SELECT strftime('%Y-%m-%dT%H:%M:%S',{UNIX_V},'unixepoch')");
    let Some(got) = run("sqlite3", &[":memory:", &q]) else {
        eprintln!("sqlite3 absent — skipping");
        return;
    };
    assert_eq!(got, tg_int("unix", UNIX_V), "sqlite3 vs timeglyph unix");
}

#[test]
fn sqlite3_julianday_agrees_on_sqlite_julian() {
    // sqlite3 is the reference implementation for the sqlite_julian format:
    // julianday('2020-01-01 00:00:00') = 2458849.5. timeglyph must decode it back.
    let Some(jd) = run(
        "sqlite3",
        &[":memory:", "SELECT julianday('2020-01-01 00:00:00')"],
    ) else {
        eprintln!("sqlite3 absent — skipping");
        return;
    };
    let v: f64 = jd.parse().unwrap();
    assert_eq!(
        tg_float("sqlite_julian", v),
        "2020-01-01T00:00:00",
        "sqlite3 julianday vs timeglyph sqlite_julian"
    );
}

#[test]
fn gnu_date_agrees_on_unix() {
    let Some(got) = run(
        "gdate",
        &["-u", "-d", &format!("@{UNIX_V}"), "+%Y-%m-%dT%H:%M:%S"],
    ) else {
        eprintln!("gdate (GNU coreutils) absent — skipping");
        return;
    };
    assert_eq!(got, tg_int("unix", UNIX_V), "GNU date vs timeglyph unix");
}

#[test]
fn cpython_agrees_on_unix_float() {
    // A Unix timestamp carried as a float (Slack `ts`, Zeek/Squid, Splunk `_time`):
    // the integer part is Unix seconds, the fraction sub-second. Distinct from the
    // integer `unix` (a float input never matched a LinearInt format before this).
    let v = 1_712_345_678.001_2_f64;
    // Committed backstop (survives without the oracle):
    assert_eq!(tg_float("unix_float", v), "2024-04-05T19:34:38");
    let code = format!(
        "import datetime;print(datetime.datetime.fromtimestamp({v},datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%S'))"
    );
    let Some(got) = run("python3", &["-c", &code]) else {
        eprintln!("python3 absent — skipping");
        return;
    };
    assert_eq!(got, tg_float("unix_float", v), "CPython vs timeglyph unix_float");
}
