//! Second independent oracle for the embedded-ID family: **unfurl** (Ryan
//! Benson, `dfir-unfurl`). `time-decode` is the primary differential oracle;
//! unfurl cross-checks the Twitter/Discord snowflake decoders from a genuinely
//! separate implementation, raising them from single- to dual-oracle. Env-gated:
//! skips cleanly when `unfurl` is not on PATH (like the time-decode battery).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

/// The largest `Timestamp: <ms>` unfurl reports for `url`, in Unix milliseconds.
/// A URL can contain several snowflakes (a Discord `channels/guild/channel/msg`
/// URL decodes all three; the guild/channel dummies land on the epoch), so the
/// actual message ID's time is the maximum. `None` if unfurl is absent / silent.
fn unfurl_timestamp_ms(url: &str) -> Option<i64> {
    let out = Command::new("unfurl").arg(url).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter(|l| l.contains("Timestamp: "))
        .filter_map(|l| {
            l.rsplit("Timestamp: ")
                .next()?
                .split_whitespace()
                .next()?
                .parse::<i64>()
                .ok()
        })
        .max()
}

/// timeglyph's own decode of `value` under `format_id`, in Unix milliseconds.
fn tg_ms(format_id: &str, value: i64) -> i64 {
    let inst = timeglyph::format(format_id)
        .unwrap()
        .decode_int(value)
        .unwrap();
    (inst.0 / 1_000_000) as i64
}

#[test]
fn unfurl_agrees_on_twitter_snowflake() {
    let value = 1_541_815_603_606_036_480;
    let Some(ms) = unfurl_timestamp_ms(&format!("https://twitter.com/x/status/{value}")) else {
        eprintln!("unfurl unavailable — skipping (install dfir-unfurl to run)");
        return;
    };
    assert_eq!(
        ms,
        tg_ms("snowflake", value),
        "unfurl vs timeglyph snowflake (ms)"
    );
}

#[test]
fn unfurl_agrees_on_discord_snowflake() {
    let value = 175_928_847_299_117_063;
    let Some(ms) = unfurl_timestamp_ms(&format!("https://discord.com/channels/1/2/{value}")) else {
        eprintln!("unfurl unavailable — skipping (install dfir-unfurl to run)");
        return;
    };
    assert_eq!(
        ms,
        tg_ms("discord", value),
        "unfurl vs timeglyph discord (ms)"
    );
}
