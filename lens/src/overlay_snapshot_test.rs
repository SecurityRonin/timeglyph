//! Offscreen (headless wgpu) egui_kittest render gate for the live overlay.
//!
//! A deterministic, GPU-offscreen GUI test: it drives the real [`LensApp`]
//! through eframe under egui_kittest's wgpu renderer and asserts each rendered
//! frame shows genuine content — not the all-black / uniform / tofu frame of a
//! failed render — via [`timeglyph_lens::shot::pixels_are_meaningful`], then
//! saves a reference PNG. This is the automated, always-on companion to the
//! `shotcheck` example (which judges a captured screenshot by the same rule).
//!
//! Two states are rendered: the empty hover-prompt screen and a decoded reading.
//!
//! Placement: this lives in the `timeglyph-lens` *binary* crate, as a private
//! `#[cfg(test)]` submodule of `overlay`, rather than in `tests/snapshot.rs`.
//! `LensApp` and the `install_fonts` / `install_theme` / `load_*` helpers are
//! private to the binary; an integration test under `tests/` links only the
//! `timeglyph_lens` library and cannot reach them (moving the overlay + its
//! `unsafe`-FFI picker into the library would be a far larger change and alter
//! the shipped rendering path). The reference PNGs still land in
//! `tests/snapshots/` — egui_kittest keys the output path off `CARGO_MANIFEST_DIR`
//! — and `cargo test` runs this as the CI gate exactly like an integration test.

use std::sync::{Arc, Mutex};

use egui_kittest::Harness;
use timeglyph_lens::settings::PersistedSettings;
use timeglyph_lens::theme::ThemePreference;

use super::{install_fonts, install_theme, load_logo, load_png_texture, LensApp, Theme};

/// Build the harness at the lens's window size, render until the decode + egui's
/// two-pass layout settle, assert the frame is meaningful, and save/compare the
/// reference snapshot. `hovered` seeds the cursor-text mutex the app reads each
/// frame (empty = the hover prompt; a number = a decoded reading).
fn gate(hovered: &str, name: &str) {
    let latest = Arc::new(Mutex::new(hovered.to_string()));

    // Mirror `overlay::run`'s Context setup (fonts + theme) so the test renders
    // the same way the shipped app does — without the fonts, every glyph is tofu.
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(680.0, 448.0))
        .build_eframe(move |cc| {
            install_fonts(&cc.egui_ctx);
            install_theme(&cc.egui_ctx, &Theme::default().palette());
            LensApp::new(
                latest,
                0,
                load_logo(&cc.egui_ctx),
                load_png_texture(
                    &cc.egui_ctx,
                    "sr-dark",
                    include_bytes!("../assets/securityronin-dark.png"),
                ),
                load_png_texture(
                    &cc.egui_ctx,
                    "sr-light",
                    include_bytes!("../assets/securityronin-light.png"),
                ),
                // Hermetic settings: a fixed snapshot so the footer zone, the
                // decoded readings, and the alt-calendar columns render identically
                // on every host — never the machine's persisted zone/theme/
                // calendars (which is what made this gate fail off the authoring
                // machine). Zone is UTC and calendars are the defaults; theme is
                // pinned to Dark so it doesn't depend on the headless runner's
                // reported system theme.
                PersistedSettings {
                    theme: ThemePreference::Dark,
                    zone_spec: "UTC".to_string(),
                    ..PersistedSettings::default()
                },
            )
        });

    // The app re-arms a repaint every frame, so run a fixed number of frames
    // rather than "until idle": frame 1 ingests + decodes, the next few settle
    // the galley-sized layout. Deterministic and headless.
    for _ in 0..8 {
        harness.step();
    }

    // The real cross-platform gate: a failed render (all-black / tofu / uniform)
    // makes `meaningful` false regardless of platform font/AA differences.
    let img = harness
        .render()
        .expect("egui_kittest wgpu offscreen render failed (no GPU adapter?)");
    let (w, h) = (img.width() as usize, img.height() as usize);
    let verdict = timeglyph_lens::shot::pixels_are_meaningful(img.as_raw(), w, h)
        .expect("rendered pixel buffer size mismatch");
    assert!(
        verdict.meaningful,
        "{name}: render is not meaningful (all-black / tofu / uniform): \
         non_black_fraction={:.4} luma_stddev={:.2}",
        verdict.non_black_fraction, verdict.luma_stddev
    );

    // The visual regression reference. Threshold is set tolerantly in
    // `kittest.toml`; see this test's module docs and the PR notes on the
    // font/renderer divergence between the macOS reference and a Linux CI runner.
    harness.snapshot(name);
}

/// The empty state: nothing under the cursor, so the overlay shows its hover
/// prompt (or the Accessibility-grant prompt on an ungranted macOS host).
#[test]
fn lens_empty() {
    gate("", "lens_empty");
}

/// A hovered Unix timestamp decodes and the overlay renders its ranked readings.
#[test]
fn lens_hover_decode() {
    gate("1721000000", "lens_hover_decode");
}
