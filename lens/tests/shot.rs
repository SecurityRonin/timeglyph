//! Screenshot meaningfulness checks (the automated half of GUI screenshot
//! validation). Property tests on the pure judge: an all-black or uniform frame
//! is a failed render; varied content is a real one; a truncated buffer errors.

use timeglyph_lens::shot::pixels_are_meaningful;

/// A solid `w×h` RGBA fill.
fn solid(w: usize, h: usize, r: u8, g: u8, b: u8) -> Vec<u8> {
    (0..w * h).flat_map(|_| [r, g, b, 255]).collect()
}

#[test]
fn all_black_frame_is_not_meaningful() {
    // The exact regression: a window painted entirely black.
    let px = solid(64, 40, 0, 0, 0);
    let v = pixels_are_meaningful(&px, 64, 40).expect("valid buffer");
    assert!(!v.meaningful, "all-black must be flagged: {v:?}");
    assert!(v.non_black_fraction < 0.001, "{v:?}");
}

#[test]
fn uniform_gray_frame_is_not_meaningful() {
    // No content, just a flat fill — still a render failure even if not black,
    // because there is zero luminance variation.
    let px = solid(64, 40, 128, 128, 128);
    let v = pixels_are_meaningful(&px, 64, 40).expect("valid buffer");
    assert!(!v.meaningful, "uniform frame must be flagged: {v:?}");
}

#[test]
fn varied_content_is_meaningful() {
    // A checkerboard of near-black + bright cells: real UI content has both
    // brightness and spread.
    let (w, h) = (64usize, 40usize);
    let mut px = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let on = (x / 4 + y / 4) % 2 == 0;
            let c = if on { 230 } else { 10 };
            px.extend_from_slice(&[c, c, c, 255]);
        }
    }
    let v = pixels_are_meaningful(&px, w, h).expect("valid buffer");
    assert!(v.meaningful, "varied content must pass: {v:?}");
    assert!(v.non_black_fraction > 0.2, "{v:?}");
    assert!(v.luma_stddev > 20.0, "{v:?}");
}

#[test]
fn size_mismatch_errors() {
    // Fail loud, never guess, on a buffer that doesn't match the dimensions
    // (a truncated / partial screen capture).
    let px = solid(10, 10, 5, 5, 5);
    assert!(pixels_are_meaningful(&px, 64, 40).is_err());
}
