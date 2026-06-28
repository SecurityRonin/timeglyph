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
use timeglyph::csv_enrich::{Conversion, EnrichOptions};
use timeglyph::interpret::{self, Candidate};
use timeglyph::RenderZone;

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
    /// Render dates in this timezone instead of UTC. Accepts `UTC`, a fixed
    /// offset (`+08:00`, `-0500`), or an IANA name (`America/New_York`). The
    /// instant is unchanged — only the displayed offset differs.
    #[arg(long, global = true, value_name = "ZONE")]
    tz: Option<String>,
    /// An artifact/source hint (e.g. `"chrome history"`, `"ntfs mft"`) that nudges
    /// identify readings whose format family matches it. A hint never hides a
    /// reading — it only adjusts the rank.
    #[arg(long, global = true, value_name = "HINT")]
    artifact: Option<String>,
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
    /// Enrich a CSV: add a human-readable column for each timestamp column.
    Csv {
        /// CSV file path, or `-` for stdin.
        path: String,
        /// Explicit conversion `COLUMN:FORMAT` (repeatable, e.g. `created:filetime`).
        #[arg(long = "convert", value_name = "COL:FMT")]
        convert: Vec<String>,
        /// Auto-detect numeric timestamp columns (the default when no --convert).
        #[arg(long)]
        auto: bool,
        /// Replace the source column in place instead of adding one to its right.
        #[arg(long)]
        replace: bool,
        /// Write output here instead of stdout.
        #[arg(short, long, value_name = "FILE")]
        output: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // Resolve the output zone once, up front: a bad --tz must fail loudly before
    // any rendering, never silently fall back to UTC.
    let zone = match RenderZone::parse(cli.tz.as_deref().unwrap_or("")) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(EXIT_ERR);
        }
    };
    let code = match cli.command {
        Some(Commands::Identify { value, json }) => {
            run_identify(value, json, &zone, cli.artifact.as_deref())
        }
        Some(Commands::Decode { format, value }) => run_decode(&format, &value, &zone),
        Some(Commands::Encode { format, datetime }) => run_encode(&format, &datetime),
        Some(Commands::Hex { bytes }) => run_hex(&bytes, &zone),
        Some(Commands::String { text }) => run_string(&text, &zone),
        Some(Commands::List) => run_list(),
        Some(Commands::Csv {
            path,
            convert,
            auto,
            replace,
            output,
        }) => run_csv(&path, &convert, auto, replace, output.as_deref(), &zone),
        None => {
            if let Some(v) = cli.value {
                run_identify(v, cli.json, &zone, cli.artifact.as_deref())
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

fn run_identify(value: i64, json: bool, zone: &RenderZone, artifact: Option<&str>) -> u8 {
    let ctx = interpret::InterpretContext {
        artifact,
        ..Default::default()
    };
    let mut cands = interpret::interpret_int_with_context(value, &ctx);
    // Re-render each `rendered` field in the requested zone. The canonical
    // `instant` (nanoseconds) stays the absolute anchor; only the human-facing
    // string changes — serialized directly (serde_json's intermediate Value
    // can't hold the i128 instant; the Serializer can).
    render_candidates_in_zone(&mut cands, zone);
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
    print_candidates(&cands, zone);
    ambiguity_code(&cands)
}

/// Overwrite each candidate's `rendered` string with its instant rendered in
/// `zone` (leaving it untouched when the instant is out of civil range).
fn render_candidates_in_zone(cands: &mut [Candidate], zone: &RenderZone) {
    for c in cands {
        if let Some(rendered) = c.instant.render(zone) {
            c.rendered = Some(rendered);
        }
    }
}

fn run_decode(format: &str, value: &str, zone: &RenderZone) -> u8 {
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
        let sentinel = interpret::sentinel_reason(v);
        if let Ok(instant) = f.decode_int(v) {
            print_decode(f, value, instant, zone);
            return sentinel_exit(v, sentinel);
        } else if let Some(reason) = sentinel {
            // e.g. 0x7FFFFFFFFFFFFFFF ("never") overflows the decode but is itself
            // a meaningful sentinel — report it rather than a generic error.
            eprintln!("warning: {v} is a likely sentinel ({reason}) — 'unset'/'never', not a real instant");
            return EXIT_AMBIGUOUS;
        }
        // a non-sentinel integer that did not decode falls through to the float path.
    }
    if let Ok(v) = value.parse::<f64>() {
        match f.decode_float(v) {
            Ok(instant) => return print_decode(f, value, instant, zone),
            Err(e) => {
                eprintln!("error: {e}");
                return EXIT_ERR;
            }
        }
    }
    eprintln!("error: could not decode {value:?} as {format}");
    EXIT_ERR
}

fn print_decode(
    f: &timeglyph::Format,
    value: &str,
    instant: timeglyph::PosixNs,
    zone: &RenderZone,
) -> u8 {
    let rendered = instant
        .render(zone)
        .unwrap_or_else(|| "<out of civil range>".into());
    let caveat = if matches!(f.tz, timeglyph::TzSemantics::LocalNaive) {
        "  (LOCAL naive — not UTC)"
    } else {
        ""
    };
    println!("{}  {value}  ->  {rendered}{caveat}", f.id);
    EXIT_OK
}

/// Exit code for a single-format decode given its sentinel classification: a
/// sentinel raw value warns and signals "review needed" (`2`), never a confident `0`.
fn sentinel_exit(value: i64, sentinel: Option<&str>) -> u8 {
    if let Some(reason) = sentinel {
        eprintln!(
            "warning: {value} is a likely sentinel ({reason}) — 'unset'/'never', not a real instant"
        );
        EXIT_AMBIGUOUS
    } else {
        EXIT_OK
    }
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

fn run_hex(bytes: &str, zone: &RenderZone) -> u8 {
    match interpret::interpret_hex(bytes) {
        Ok(groups) => {
            let mut any = false;
            let mut has_sentinel = false;
            for (layout, cands) in &groups {
                println!("# byte layout: {layout}");
                print_candidates(cands, zone);
                any |= !cands.is_empty();
                has_sentinel |= cands.iter().any(|c| c.sentinel);
            }
            // Pipeline safety: no readings, or any sentinel reading → review needed.
            if any && !has_sentinel {
                EXIT_OK
            } else {
                EXIT_AMBIGUOUS
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            EXIT_ERR
        }
    }
}

fn run_string(text: &str, zone: &RenderZone) -> u8 {
    let cands = interpret::interpret_string(text);
    if cands.is_empty() {
        eprintln!("error: {text:?} did not parse as any known string timestamp form");
        return EXIT_ERR;
    }
    println!("# readings consistent with {text:?}:");
    print_candidates(&cands, zone);
    EXIT_OK
}

fn run_list() -> u8 {
    for f in timeglyph::registry::FORMATS {
        println!("{:<16} {:<48} {}", f.id, f.label, f.citation);
    }
    EXIT_OK
}

fn run_csv(
    path: &str,
    convert: &[String],
    auto: bool,
    replace: bool,
    output: Option<&str>,
    zone: &RenderZone,
) -> u8 {
    let input = if path == "-" {
        let mut s = String::new();
        if std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).is_err() {
            eprintln!("error: failed to read stdin");
            return EXIT_ERR;
        }
        s
    } else {
        match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {path}: {e}");
                return EXIT_ERR;
            }
        }
    };
    let mut conversions = Vec::new();
    for c in convert {
        match c.split_once(':') {
            Some((col, fmt)) if !col.is_empty() && !fmt.is_empty() => {
                conversions.push(Conversion {
                    column: col.to_string(),
                    format: fmt.to_string(),
                });
            }
            _ => {
                eprintln!("error: --convert expects COLUMN:FORMAT, got {c:?}");
                return EXIT_ERR;
            }
        }
    }
    // Auto-detect by default when no explicit conversion was requested.
    let auto = auto || conversions.is_empty();
    let opts = EnrichOptions {
        conversions,
        auto,
        replace,
        zone: zone.clone(),
    };
    match timeglyph::csv_enrich::enrich(&input, &opts) {
        Ok(out) => {
            if let Some(path) = output {
                if let Err(e) = std::fs::write(path, out) {
                    eprintln!("error: cannot write {path}: {e}");
                    return EXIT_ERR;
                }
            } else {
                print!("{out}");
            }
            EXIT_OK
        }
        Err(e) => {
            eprintln!("error: {e}");
            EXIT_ERR
        }
    }
}

fn print_candidates(cands: &[Candidate], zone: &RenderZone) {
    if cands.is_empty() {
        println!("  (no plausible interpretation)");
        return;
    }
    for c in cands {
        let flag = if c.sentinel { " [sentinel]" } else { "" };
        let rendered = c.instant.render(zone).or_else(|| c.rendered.clone());
        println!(
            "  [{:.2}] {:<16} {}  ({}){flag}",
            c.score,
            c.format_id,
            rendered.as_deref().unwrap_or("<out of range>"),
            c.label,
        );
    }
}
