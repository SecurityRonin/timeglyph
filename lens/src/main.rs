//! `timeglyph-lens` binary.
//!
//! Two modes:
//! - **GUI overlay** (default): an always-on-top window that follows the cursor;
//!   whatever UI element you hover, any number in its text is shown with
//!   timeglyph's ranked datetime readings (Spy++-style).
//! - **Live console** (`--live`): prints the element under the cursor and its
//!   readings to the terminal instead of the overlay.
//!
//! It takes no text argument — one-shot text decoding lives in `timeglyph scan`
//! (both call the same `scan` core), so the duplicate mode was dropped.

use timeglyph::RenderZone;
use timeglyph_lens::scan;

mod macmenu;
mod overlay;
mod picker;

fn main() {
    // Parse flags out of the args: -v / -vv / --verbose (verbosity level), --live
    // (console inspector). The lens takes no positional text argument.
    let mut verbose: u8 = 0;
    let mut live = false;
    let mut positional: Vec<String> = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--live" => live = true,
            "--verbose" => verbose = verbose.saturating_add(1),
            // -v, -vv, -vvv … → verbosity = number of v's.
            s if s.len() >= 2 && s.starts_with('-') && s[1..].bytes().all(|b| b == b'v') => {
                verbose = verbose.saturating_add((s.len() - 1) as u8);
            }
            _ => positional.push(arg),
        }
    }

    // Text decoding is `timeglyph scan`'s job; fail loudly rather than silently
    // launch the GUI while ignoring an argument the user clearly meant to decode.
    if !positional.is_empty() {
        eprintln!(
            "timeglyph-lens takes no text argument; use `timeglyph scan <text>` (or a file) \
             for text decoding, or run with no args for the overlay / --live for the console \
             inspector"
        );
        std::process::exit(1);
    }

    if live {
        // Live console mode: print the element under the cursor and its readings.
        live_console();
    } else if let Err(e) = overlay::run(verbose) {
        // Default: the always-on-top GUI overlay. -v logs activity to stderr; -vv
        // also shows the raw element text under the cursor in the panel.
        eprintln!("timeglyph-lens: {e}");
        eprintln!(
            "(try `timeglyph-lens --live` for the console inspector, or -v / -vv for verbose)"
        );
        std::process::exit(1);
    }
}

/// Poll the element under the cursor and print its timeglyph readings.
fn live_console() {
    let picker = match picker::Picker::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("timeglyph-lens: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("timeglyph-lens: watching the cursor (Ctrl-C to stop)…");
    let mut last = String::new();
    loop {
        let text = picker.text_under_cursor().unwrap_or_default();
        if text != last {
            last.clone_from(&text);
            let hits = scan::inspect_text(&text, 4, &RenderZone::Utc);
            println!("\nelement: {text:?}");
            for nr in hits {
                println!("  {}", nr.number);
                for r in nr.readings {
                    println!("      {r}");
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
