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
    let n = width
        .checked_mul(height)
        .ok_or_else(|| "image dimensions overflow".to_string())?;
    if n == 0 {
        return Err("empty image (zero width or height)".to_string());
    }
    let need = n
        .checked_mul(4)
        .ok_or_else(|| "image buffer size overflows usize".to_string())?;
    if rgba.len() < need {
        return Err(format!(
            "rgba buffer is {} bytes, need {need} for {width}x{height} (truncated capture?)",
            rgba.len()
        ));
    }

    // Rec. 601 luma; a pixel counts as non-black only well above sensor/JPEG-style
    // noise so a truly black frame reads as ~0% non-black, not a few stray pixels.
    const NEAR_BLACK_LUMA: f64 = 24.0;
    let mut non_black = 0usize;
    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    for i in 0..n {
        let p = i * 4;
        let luma = 0.299 * f64::from(rgba[p])
            + 0.587 * f64::from(rgba[p + 1])
            + 0.114 * f64::from(rgba[p + 2]);
        if luma > NEAR_BLACK_LUMA {
            non_black += 1;
        }
        sum += luma;
        sum_sq += luma * luma;
    }

    let count = n as f64;
    let mean = sum / count;
    let luma_stddev = (sum_sq / count - mean * mean).max(0.0).sqrt();
    let non_black_fraction = non_black as f64 / count;

    // Real content needs BOTH: some genuinely lit pixels AND luminance spread. The
    // AND rejects an all-black frame (no lit pixels) and a uniform fill of any
    // shade (no spread) — both failed renders.
    let meaningful = non_black_fraction > 0.01 && luma_stddev > 5.0;

    Ok(Verdict {
        non_black_fraction,
        luma_stddev,
        meaningful,
    })
}
