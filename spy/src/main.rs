//! `timeglyph-spy` binary.
//!
//! Two modes:
//! - **Text mode** (any platform): `timeglyph-spy "<text>"` decodes every number
//!   in the argument — how the scan core is driven without a desktop.
//! - **Live mode** (Windows): with no arguments, opens an always-on-top window
//!   that follows the cursor; whatever UI element you hover, any number in its
//!   text is shown with timeglyph's ranked datetime readings (Spy++-style).

use timeglyph::RenderZone;
use timeglyph_spy::scan;

mod macmenu;
mod overlay;
mod picker;

fn main() {
    // Parse flags out of the args: -v / -vv / --verbose (verbosity level), --live
    // (console inspector). Everything else is positional text to decode.
    let mut verbose: u8 = 0;
    let mut live = false;
    let mut text: Vec<String> = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--live" => live = true,
            "--verbose" => verbose = verbose.saturating_add(1),
            // -v, -vv, -vvv … → verbosity = number of v's.
            s if s.len() >= 2 && s.starts_with('-') && s[1..].bytes().all(|b| b == b'v') => {
                verbose = verbose.saturating_add((s.len() - 1) as u8);
            }
            _ => text.push(arg),
        }
    }

    if live {
        // Live console mode: print the element under the cursor and its readings.
        live_console();
    } else if !text.is_empty() {
        // Text mode (any platform): decode every number in the argument string.
        for nr in scan::inspect_text(&text.join(" "), 6, &RenderZone::Utc) {
            println!("{}", nr.number);
            for r in &nr.readings {
                println!("    {r}");
            }
        }
    } else if let Err(e) = overlay::run(verbose) {
        // Default: the always-on-top GUI overlay. -v logs activity to stderr; -vv
        // also shows the raw element text under the cursor in the panel.
        eprintln!("timeglyph-spy: {e}");
        eprintln!(
            "(try `timeglyph-spy --live` for the console inspector, text to decode, \
             or -v / -vv for verbose)"
        );
        std::process::exit(1);
    }
}

/// Poll the element under the cursor and print its timeglyph readings.
fn live_console() {
    let picker = match picker::Picker::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("timeglyph-spy: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("timeglyph-spy: watching the cursor (Ctrl-C to stop)…");
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
