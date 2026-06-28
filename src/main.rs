//! `timeglyph` CLI — a thin Humble-Object shell over the library engine.
//!
//! Subcommands: `identify` (the safe default; ranked candidates — a raw value is
//! usually underdetermined), `decode <format> <value>`, `encode <format> <dt>`,
//! `hex <bytes>`, `string <text>`, `list`. A bare value is a back-compat shortcut
//! for `identify`. Exit codes are pipeline-safe: `0` ok, `2` ambiguous or a
//! sentinel (review needed), `1` error.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use timeglyph::interpret::{self, Candidate};

const EXIT_OK: u8 = 0;
const EXIT_ERR: u8 = 1;
const EXIT_AMBIGUOUS: u8 = 2;

#[derive(Parser, Debug)]
#[command(name = "timeglyph", version, about = "Forensic timestamp decipherment")]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    /// A value to IDENTIFY (back-compat shortcut for `identify <value>`).
    value: Option<i64>,
    /// Emit JSON (with the bare-value shortcut).
    #[arg(long)]
    json: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Identify a value across all formats (ranked candidates, never one verdict).
    Identify {
        /// The value to identify.
        value: i64,
        /// Emit JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Decode a value under ONE known format id (see `list`).
    Decode {
        /// Format id (e.g. `filetime`, `unix`, `gps` with the `leap` feature).
        format: String,
        /// The value (decimal integer, or a float for float formats).
        value: String,
    },
    /// Encode a datetime (ISO 8601 / RFC 3339 / ASN.1) into a format id.
    Encode {
        /// Target format id.
        format: String,
        /// The datetime string to encode.
        datetime: String,
    },
    /// Decode raw hex bytes (LE/BE widths + packed on-disk layouts).
    Hex {
        /// Hex bytes (whitespace/`:`/`0x` tolerated).
        bytes: String,
    },
    /// Parse a string timestamp (ISO 8601 / RFC 3339 / ASN.1 UTCTime/GeneralizedTime).
    String {
        /// The timestamp string.
        text: String,
    },
    /// List every registered format with its citation.
    List,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let code = match cli.command {
        Some(Commands::Identify { value, json }) => run_identify(value, json),
        Some(Commands::Decode { format, value }) => run_decode(&format, &value),
        Some(Commands::Encode { format, datetime }) => run_encode(&format, &datetime),
        Some(Commands::Hex { bytes }) => run_hex(&bytes),
        Some(Commands::String { text }) => run_string(&text),
        Some(Commands::List) => run_list(),
        None => {
            if let Some(v) = cli.value {
                run_identify(v, cli.json)
            } else {
                eprintln!("error: give a VALUE or a subcommand (see --help)");
                EXIT_ERR
            }
        }
    };
    ExitCode::from(code)
}

/// Exit code reflecting interpretation confidence (pipeline safety): a sentinel
/// top reading or a tie for the top score is "review needed" (`2`); a clear
/// single winner is `0`; no readings is `2` (nothing confident).
fn ambiguity_code(cands: &[Candidate]) -> u8 {
    let Some(top) = cands.first() else {
        return EXIT_AMBIGUOUS;
    };
    if top.sentinel {
        return EXIT_AMBIGUOUS;
    }
    if cands.len() >= 2 && (top.score - cands[1].score).abs() < 1e-9 {
        return EXIT_AMBIGUOUS;
    }
    EXIT_OK
}

fn run_identify(value: i64, json: bool) -> u8 {
    let cands = interpret::interpret_int(value);
    if json {
        match serde_json::to_string_pretty(&cands) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: serializing candidates: {e}");
                return EXIT_ERR;
            }
        }
        return ambiguity_code(&cands);
    }
    println!(
        "# readings consistent with {value} (ranked; a raw value is usually \
         underdetermined — not a single verdict):"
    );
    print_candidates(&cands);
    ambiguity_code(&cands)
}

fn run_decode(format: &str, value: &str) -> u8 {
    // Leap-aware scales (gps/tai64/ntp) decode separately — never via PosixNs.
    #[cfg(feature = "leap")]
    if let Ok(v) = value.parse::<i64>() {
        if let Some(result) = timeglyph::leap::decode(format, v) {
            return match result {
                Ok(r) => {
                    println!(
                        "{}  {value}  ->  {}  (leap-correct UTC)",
                        r.scale, r.utc_rfc3339
                    );
                    for a in &r.assumptions {
                        println!("    - {a}");
                    }
                    EXIT_OK
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    EXIT_ERR
                }
            };
        }
    }
    let f = match timeglyph::format(format) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            return EXIT_ERR;
        }
    };
    // Integer value first; fall back to a float for float-encoded formats.
    if let Ok(v) = value.parse::<i64>() {
        if let Ok(instant) = f.decode_int(v) {
            return print_decode(f, value, instant);
        }
    }
    if let Ok(v) = value.parse::<f64>() {
        match f.decode_float(v) {
            Ok(instant) => return print_decode(f, value, instant),
            Err(e) => {
                eprintln!("error: {e}");
                return EXIT_ERR;
            }
        }
    }
    eprintln!("error: could not decode {value:?} as {format}");
    EXIT_ERR
}

fn print_decode(f: &timeglyph::Format, value: &str, instant: timeglyph::PosixNs) -> u8 {
    let rendered = instant
        .to_rfc3339()
        .unwrap_or_else(|| "<out of civil range>".into());
    let caveat = if matches!(f.tz, timeglyph::TzSemantics::LocalNaive) {
        "  (LOCAL naive — not UTC)"
    } else {
        ""
    };
    println!("{}  {value}  ->  {rendered}{caveat}", f.id);
    EXIT_OK
}

fn run_encode(format: &str, datetime: &str) -> u8 {
    let Some(instant) = interpret::interpret_string(datetime)
        .first()
        .map(|c| c.instant)
    else {
        eprintln!("error: could not parse datetime {datetime:?} (try ISO 8601 / RFC 3339)");
        return EXIT_ERR;
    };
    let f = match timeglyph::format(format) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            return EXIT_ERR;
        }
    };
    match f.encode_int(instant) {
        Ok(v) => {
            println!("{v}");
            EXIT_OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            EXIT_ERR
        }
    }
}

fn run_hex(bytes: &str) -> u8 {
    match interpret::interpret_hex(bytes) {
        Ok(groups) => {
            for (layout, cands) in groups {
                println!("# byte layout: {layout}");
                print_candidates(&cands);
            }
            EXIT_OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            EXIT_ERR
        }
    }
}

fn run_string(text: &str) -> u8 {
    let cands = interpret::interpret_string(text);
    if cands.is_empty() {
        eprintln!("error: {text:?} did not parse as any known string timestamp form");
        return EXIT_ERR;
    }
    println!("# readings consistent with {text:?}:");
    print_candidates(&cands);
    EXIT_OK
}

fn run_list() -> u8 {
    for f in timeglyph::registry::FORMATS {
        println!("{:<16} {:<48} {}", f.id, f.label, f.citation);
    }
    EXIT_OK
}

fn print_candidates(cands: &[Candidate]) {
    if cands.is_empty() {
        println!("  (no plausible interpretation)");
        return;
    }
    for c in cands {
        let flag = if c.sentinel { " [sentinel]" } else { "" };
        println!(
            "  [{:.2}] {:<16} {}  ({}){flag}",
            c.score,
            c.format_id,
            c.rendered.as_deref().unwrap_or("<out of range>"),
            c.label,
        );
    }
}
