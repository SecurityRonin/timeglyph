//! `shotcheck <screenshot.png>` — the automated verdict for GUI screenshot
//! validation (see `scripts/screenshot-validate.*`).
//!
//! Decodes a captured lens-window PNG and exits 0 if it shows real rendered
//! content, 1 if it is an all-black / uniform frame (the failed-render
//! regression), 2 on a usage/IO/decode error. Kept an example so its image-decode
//! dependency is dev-only and never links into the shipped lens binary; the
//! pixel judgment itself lives in the unit-tested `timeglyph_lens::shot` module.

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: shotcheck <screenshot.png>");
        return ExitCode::from(2);
    };

    let rgba = match image::open(&path) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            eprintln!("shotcheck: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);

    match timeglyph_lens::shot::pixels_are_meaningful(rgba.as_raw(), w, h) {
        Ok(v) => {
            eprintln!(
                "shotcheck {path}: {w}x{h}  non_black={:.1}%  luma_stddev={:.1}  => {}",
                v.non_black_fraction * 100.0,
                v.luma_stddev,
                if v.meaningful {
                    "MEANINGFUL"
                } else {
                    "ALL-BLACK/UNIFORM — render failed"
                }
            );
            if v.meaningful {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("shotcheck: {e}");
            ExitCode::from(2)
        }
    }
}
