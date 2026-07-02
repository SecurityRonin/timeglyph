//! The live overlay (eframe/egui): an always-on-top window that follows the
//! cursor and shows timeglyph's readings for any number in the element under it.
//! Cross-platform — the same window on Windows and macOS; only the [`Picker`] is
//! platform-specific.
//!
//! Presentation is a compact "time-instrument" inspector panel: each number is
//! the subject, its candidate readings a clean `format · instant · label` row,
//! and the raw source element a de-emphasised caption. High-contrast warm-dark
//! palette (targets WCAG AA on `BG_DEEP`) with a single brass/amber accent.

use std::time::Duration;

use eframe::egui;
use egui::{Color32, FontId, Frame, Margin, RichText, Rounding, Stroke};
use timeglyph::RenderZone;

use crate::picker::Picker;
use crate::scan::{self, NumberReadings, Reading};

// Warm "time instrument" palette. Text contrasts vs BG_DEEP (#14120F):
// INK ~15:1, AMBER ~10:1, MUTE ~8:1, FAINT ~5:1 — all clear WCAG AA.
const BG_DEEP: Color32 = Color32::from_rgb(20, 18, 15); // warm near-black
const BG_CARD: Color32 = Color32::from_rgb(31, 28, 22);
const BG_CHIP: Color32 = Color32::from_rgb(38, 31, 20); // amber-tinted
const HAIRLINE: Color32 = Color32::from_rgb(52, 47, 38);
const INK: Color32 = Color32::from_rgb(245, 241, 232); // warm white — datetime values
const AMBER: Color32 = Color32::from_rgb(240, 180, 41); // brass accent — format + wordmark
const MUTE: Color32 = Color32::from_rgb(179, 169, 145); // labels
const FAINT: Color32 = Color32::from_rgb(143, 134, 116); // captions
const GLYPH: Color32 = Color32::from_rgb(92, 82, 64); // large empty-state mark

/// How many readings to show per number.
const MAX_READINGS: usize = 4;

/// Open the overlay window and run until it is closed.
pub fn run() -> Result<(), String> {
    let picker = Picker::new()?;
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 380.0])
            .with_min_inner_size([360.0, 200.0])
            .with_always_on_top()
            .with_title("timeglyph-spy"),
        ..Default::default()
    };
    eframe::run_native(
        "timeglyph-spy",
        native_options,
        Box::new(|cc| {
            install_theme(&cc.egui_ctx);
            Ok(Box::new(SpyApp::new(picker)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// Install the panel's warm-dark theme once at startup.
fn install_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_DEEP;
    visuals.window_fill = BG_DEEP;
    visuals.override_text_color = Some(INK);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, HAIRLINE);
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| s.spacing.item_spacing = egui::vec2(8.0, 6.0));
}

struct SpyApp {
    picker: Picker,
    last_text: String,
    /// The raw element text under the cursor (shown as a de-emphasised caption).
    source: String,
    /// The decoded model: numbers and their ranked readings.
    hits: Vec<NumberReadings>,
}

impl SpyApp {
    fn new(picker: Picker) -> Self {
        Self {
            picker,
            last_text: String::new(),
            source: String::new(),
            hits: Vec::new(),
        }
    }
}

impl eframe::App for SpyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let text = self.picker.text_under_cursor().unwrap_or_default();
        if text != self.last_text {
            self.last_text.clone_from(&text);
            self.source = text.clone();
            self.hits = scan::inspect_text(&text, MAX_READINGS, &RenderZone::Utc);
        }

        let panel = Frame::none()
            .fill(BG_DEEP)
            .inner_margin(Margin::symmetric(16.0, 14.0));
        egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
            header(ui, &self.source);
            ui.separator();
            ui.add_space(10.0);
            if self.source.is_empty() {
                empty_state(
                    ui,
                    "Hover an element with a number",
                    "Point at any on-screen value to decode it",
                );
            } else if self.hits.is_empty() {
                empty_state(
                    ui,
                    "No timestamp-like number here",
                    "This element has no value that reads as a date",
                );
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for nr in &self.hits {
                            number_card(ui, nr);
                            ui.add_space(10.0);
                        }
                    });
            }
        });
        // Poll the cursor a few times a second without busy-spinning.
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

/// Slim header: the wordmark plus a de-emphasised, truncated caption of the raw
/// source element — context, not the subject (and it keeps sensitive surrounding
/// text from dominating the panel).
fn header(ui: &mut egui::Ui, source: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("◷ timeglyph")
                .font(FontId::monospace(15.0))
                .color(AMBER)
                .strong(),
        );
        if !source.is_empty() {
            ui.add_space(10.0);
            let caption = source.split_whitespace().collect::<Vec<_>>().join(" ");
            ui.add(
                egui::Label::new(
                    RichText::new(caption)
                        .font(FontId::proportional(11.0))
                        .color(FAINT),
                )
                .truncate(),
            );
        }
    });
}

/// One number as the subject, with its ranked readings beneath it.
fn number_card(ui: &mut egui::Ui, nr: &NumberReadings) {
    Frame::none()
        .fill(BG_CARD)
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(14.0, 12.0))
        .stroke(Stroke::new(1.0, HAIRLINE))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(&nr.number)
                    .font(FontId::monospace(21.0))
                    .color(INK)
                    .strong(),
            );
            ui.add_space(8.0);
            for (i, r) in nr.readings.iter().enumerate() {
                if i > 0 {
                    ui.add_space(8.0);
                }
                reading_row(ui, r);
            }
        });
}

/// One reading: an amber format chip and the rendered instant on one line, the
/// human label beneath. Role is conveyed by position and weight, not colour
/// alone (chip first + monospace; label last + smaller + muted).
fn reading_row(ui: &mut egui::Ui, r: &Reading) {
    ui.horizontal(|ui| {
        Frame::none()
            .fill(BG_CHIP)
            .rounding(Rounding::same(4.0))
            .inner_margin(Margin::symmetric(6.0, 2.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(&r.format_id)
                        .font(FontId::monospace(11.0))
                        .color(AMBER)
                        .strong(),
                );
            });
        ui.add_space(8.0);
        ui.label(
            RichText::new(&r.rendered)
                .font(FontId::monospace(14.0))
                .color(INK),
        );
    });
    ui.label(
        RichText::new(&r.label)
            .font(FontId::proportional(11.5))
            .color(MUTE),
    );
}

/// A calm centred placeholder instead of a debug string.
fn empty_state(ui: &mut egui::Ui, title: &str, sub: &str) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("◷")
                .font(FontId::monospace(34.0))
                .color(GLYPH),
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new(title)
                .font(FontId::proportional(15.0))
                .color(MUTE)
                .strong(),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(sub)
                .font(FontId::proportional(12.0))
                .color(FAINT),
        );
    });
}
