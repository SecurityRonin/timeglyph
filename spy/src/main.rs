//! `timeglyph-spy` binary.
//!
//! Two modes:
//! - **Text mode** (any platform): `timeglyph-spy "<text>"` decodes every number
//!   in the argument — how the scan core is driven without a desktop.
//! - **Live mode** (Windows): with no arguments, opens an always-on-top window
//!   that follows the cursor; whatever UI element you hover, any number in its
//!   text is shown with timeglyph's ranked datetime readings (Spy++-style).

use timeglyph_spy::scan;

#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod picker;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        for nr in scan::inspect_text(&args.join(" "), 6) {
            println!("{}", nr.number);
            for r in &nr.readings {
                println!("    {r}");
            }
        }
        return;
    }

    #[cfg(windows)]
    if let Err(e) = overlay::run() {
        eprintln!("timeglyph-spy: {e}");
        std::process::exit(1);
    }

    #[cfg(not(windows))]
    {
        eprintln!("timeglyph-spy: the live cursor inspector is Windows-only.");
        eprintln!("Pass text to decode the numbers in it, e.g.:");
        eprintln!("    timeglyph-spy \"cookie value 13390845530064940\"");
    }
}
