//! `timeglyph` CLI — a thin Humble-Object shell over the library engine.
//!
//! The default action (a bare value) is AUTO-DETECT, which prints *ranked
//! candidate interpretations* — never a single "detected" answer — because a raw
//! value is usually ambiguous. Use `--from <id>` to decode under one known
//! format, `--hex` for raw bytes, `--list` to see the registry.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::process::ExitCode;

use clap::Parser;
use timeglyph::interpret;

#[derive(Parser, Debug)]
#[command(name = "timeglyph", version, about = "Forensic timestamp decipherment")]
struct Cli {
    /// A timestamp value to IDENTIFY across all formats (auto-detect).
    value: Option<i64>,

    /// Decode raw hex bytes (LE/BE, 32/64-bit) instead of a decimal value.
    #[arg(long, value_name = "HEX")]
    hex: Option<String>,

    /// Decode under ONE specific format id (see --list) instead of auto-detect.
    #[arg(long, value_name = "ID")]
    from: Option<String>,

    /// List every registered format and exit.
    #[arg(long)]
    list: bool,

    /// Emit JSON instead of text.
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.list {
        for f in timeglyph::registry::FORMATS {
            println!("{:<14} {:<46} {}", f.id, f.label, f.citation);
        }
        return ExitCode::SUCCESS;
    }

    if let Some(id) = cli.from.as_deref() {
        let Some(value) = cli.value else {
            eprintln!("error: --from requires a VALUE to decode");
            return ExitCode::FAILURE;
        };
        match timeglyph::format(id).and_then(|f| f.decode_int(value)) {
            Ok(instant) => {
                println!(
                    "{id}  {value}  ->  {}",
                    instant
                        .to_rfc3339()
                        .unwrap_or_else(|| "<out of civil range>".into())
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    } else if let Some(hex) = cli.hex.as_deref() {
        match interpret::interpret_hex(hex) {
            Ok(groups) => {
                for (layout, cands) in groups {
                    println!("# byte layout: {layout}");
                    print_candidates(&cands);
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        }
    } else if let Some(value) = cli.value {
        let cands = interpret::interpret_int(value);
        if cli.json {
            // SCAFFOLD: a minimal hand-rolled JSON (no serde wiring on Candidate
            // yet — HANDOFF: derive Serialize on Candidate for real --json).
            println!("{{\"value\": {value}, \"candidates\": {}}}", cands.len());
        }
        println!("# ranked candidate interpretations of {value} (NOT a single answer):");
        print_candidates(&cands);
        ExitCode::SUCCESS
    } else {
        eprintln!("error: give a VALUE, --hex <bytes>, or --list (see --help)");
        ExitCode::FAILURE
    }
}

fn print_candidates(cands: &[interpret::Candidate]) {
    if cands.is_empty() {
        println!("  (no plausible interpretation)");
        return;
    }
    for c in cands {
        println!(
            "  [{:.2}] {:<14} {}  ({})",
            c.score,
            c.format_id,
            c.rendered.as_deref().unwrap_or("<out of range>"),
            c.label,
        );
    }
}
