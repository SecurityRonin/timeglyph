//! Screenshot meaningfulness check — the automated half of GUI screenshot
//! validation (see `scripts/screenshot-validate.*`).
//!
//! A lens window that renders correctly has varied, non-black content. The
//! all-black regression — dropping eframe's `default_fonts` empties the glyph
//! atlas and paints the whole window black, with no error (see the load-bearing
//! comment in `lens/Cargo.toml`) — produces a near-uniform black frame. This
//! judges a captured window's decoded pixels for exactly that failure, so a
//! screenshot can be validated automatically instead of only by eye.

/// Verdict on a captured screenshot's pixels.
#[derive(Debug, Clone, Copy)]
pub struct Verdict {
    /// Fraction of pixels brighter than the near-black threshold (0.0–1.0).
    pub non_black_fraction: f64,
    /// Standard deviation of per-pixel luminance (0 = a flat, uniform frame).
    pub luma_stddev: f64,
    /// True when the frame shows real content: not all-black, not uniform.
    pub meaningful: bool,
}

/// Judge whether tightly-packed RGBA pixels show meaningful rendered content
/// rather than the all-black / uniform frame of a failed render. `rgba` must be
/// `width * height * 4` bytes. Errors on a size mismatch (fail loud, never guess
/// past a truncated capture).
pub fn pixels_are_meaningful(rgba: &[u8], width: usize, height: usize) -> Result<Verdict, String> {
    let _ = (rgba, width, height);
    todo!("implemented in the GREEN commit")
}
