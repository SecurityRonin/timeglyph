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
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // Live console mode (Windows/macOS): print the element under the cursor
        // and its readings a few times — handy for verifying the picker.
        Some("--live") => live_console(),
        // Text mode (any platform): decode every number in the argument string.
        Some(_) => {
            for nr in scan::inspect_text(&args.join(" "), 6, &RenderZone::Utc) {
                println!("{}", nr.number);
                for r in &nr.readings {
                    println!("    {r}");
                }
            }
        }
        // No args: launch the always-on-top GUI overlay (the default).
        None => {
            if let Err(e) = overlay::run() {
                eprintln!("timeglyph-spy: {e}");
                eprintln!("(try `timeglyph-spy --live` for the console inspector, or pass text)");
                std::process::exit(1);
            }
        }
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
