//! The live overlay (eframe/egui): an always-on-top window that follows the
//! cursor and shows timeglyph's readings for any number in the element under it.
//! Cross-platform — the same window on Windows and macOS; only the [`Picker`] is
//! platform-specific.
//!
//! Presentation is a compact "time-instrument" inspector panel: each number is
//! the subject; each candidate reading a `format · instant · label` row with a
//! confidence badge and an optional 干支 expansion; the raw source element a
//! de-emphasised caption; and a footer that selects the display timezone and
//! opens settings (dark/light theme, whether to show 干支). Both palettes clear
//! WCAG AA (see [`timeglyph_spy::theme`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use egui::{Color32, FontId, Frame, Margin, RichText, Rounding, Stroke};
use timeglyph::{PosixNs, RenderZone};
use timeglyph_spy::theme::{Palette, Theme};
use timeglyph_spy::zone::{self, parse_zone, ZoneChoice};
use timeglyph_spy::{ganzhi, text, tzinfo, tzmap};

use crate::picker::Picker;
use crate::scan::{self, NumberReadings, Reading};

/// How many readings to show per number.
const MAX_READINGS: usize = 4;

/// Session settings (never persisted — like the zone, a prior case's preferences
/// can't silently apply to the next launch).
#[derive(Clone, Copy, Default)]
struct Settings {
    /// Dark (default) or light palette.
    theme: Theme,
    /// Whether to show the 干支 / lunisolar line (and, with it, the longitude
    /// input, which only refines the 干支 hour pillar). Off by default.
    show_lunar: bool,
}

/// Open the overlay window and run until it is closed.
pub fn run() -> Result<(), String> {
    // Fail-fast: verify the picker initializes (e.g. Accessibility permission is
    // granted) before opening the window. The poll thread builds its own picker
    // so the AX handle / COM apartment stays on that thread; this probe is
    // dropped immediately.
    let _ = Picker::new()?;
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 400.0])
            .with_min_inner_size([380.0, 220.0])
            .with_always_on_top()
            .with_title("timeglyph-spy"),
        ..Default::default()
    };
    eframe::run_native(
        "timeglyph-spy",
        native_options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            install_theme(&cc.egui_ctx, &Theme::default().palette());
            // Native macOS app menu with a standard Settings… item (⌘,).
            crate::macmenu::install();
            let latest = Arc::new(Mutex::new(String::new()));
            spawn_cursor_poll(cc.egui_ctx.clone(), Arc::clone(&latest));
            Ok(Box::new(SpyApp::new(latest)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// Poll the element under the cursor on a background thread so the render thread
/// never blocks on the (cross-process, sometimes slow) AX / UI-Automation read.
/// Writes the latest text into `latest` and wakes the UI only when it changes.
/// The picker is built here so its AX handle / COM apartment stays on this thread.
fn spawn_cursor_poll(ctx: egui::Context, latest: Arc<Mutex<String>>) {
    std::thread::spawn(move || {
        let Ok(picker) = Picker::new() else { return };
        let mut prev = String::new();
        loop {
            let text = picker.text_under_cursor().unwrap_or_default();
            if text != prev {
                prev.clone_from(&text);
                if let Ok(mut slot) = latest.lock() {
                    *slot = text;
                }
                ctx.request_repaint();
            }
            std::thread::sleep(Duration::from_millis(70));
        }
    });
}

/// Append the OS fallback fonts (a CJK face for the 干支 pillars + lunar date, a
/// symbol face for the chrome's ◷ / ⚠ / 🌐 / ⚙) to both families. egui's bundled
/// fonts carry no CJK/symbol glyphs, so without this those render as missing-glyph
/// boxes (tofu); which glyphs the stack must cover is asserted by `tests/fonts.rs`.
/// Loaded at runtime (not bundled); if the host has neither, the overlay still
/// runs and only the uncovered glyphs degrade to tofu.
fn install_fonts(ctx: &egui::Context) {
    let stack = timeglyph_spy::fonts::fallback_fonts();
    if stack.is_empty() {
        return;
    }
    let mut fonts = egui::FontDefinitions::default();
    let mut keys = Vec::new();
    for (key, bytes) in stack {
        fonts
            .font_data
            .insert(key.to_owned(), egui::FontData::from_owned(bytes));
        keys.push(key.to_owned());
    }
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let entry = fonts.families.entry(family).or_default();
        for key in &keys {
            entry.push(key.clone());
        }
    }
    ctx.set_fonts(fonts);
}

/// Apply a palette to egui's visuals (panel/window fill, default text colour,
/// hairline). Re-applied each frame so a theme switch takes effect immediately.
fn install_theme(ctx: &egui::Context, pal: &Palette) {
    let mut visuals = if pal.base_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = pal.bg_deep;
    visuals.window_fill = pal.bg_deep;
    visuals.override_text_color = Some(pal.ink);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal.hairline);
    ctx.set_visuals(visuals);
    ctx.style_mut(|s| s.spacing.item_spacing = egui::vec2(8.0, 6.0));
}

struct SpyApp {
    /// Latest text under the cursor, produced by the background poll thread; the
    /// render thread only reads this snapshot (never the AX/UIA API directly).
    latest: Arc<Mutex<String>>,
    last_text: String,
    /// The raw element text under the cursor (shown as a de-emphasised caption).
    source: String,
    /// The decoded model: numbers and their ranked readings.
    hits: Vec<NumberReadings>,
    /// The display timezone. Session-scoped, UTC by default: it never persists
    /// across launches, so a prior case's zone can't silently apply to the next.
    zone: ZoneChoice,
    /// Cached IANA continents (top level of the cascading Region → Zone picker).
    continents: Vec<String>,
    /// Optional global longitude (°E): refines every reading's 干支 hour pillar to
    /// true solar time. Its footer entry buffer.
    longitude: Option<f64>,
    longitude_input: String,
    /// Whether the clickable world map window is open, and the picked UTC offset
    /// (used to highlight the whole band, not a single polygon).
    show_map: bool,
    map_pick: Option<f64>,
    /// Whether the settings window is open. Shared with its viewport, which flips
    /// it false on close.
    show_settings: Arc<AtomicBool>,
    /// Session settings (theme, whether to show 干支). Shared with the settings
    /// viewport so its controls write back to the main window.
    settings: Arc<Mutex<Settings>>,
}

impl SpyApp {
    fn new(latest: Arc<Mutex<String>>) -> Self {
        Self {
            latest,
            last_text: String::new(),
            source: String::new(),
            hits: Vec::new(),
            zone: ZoneChoice::default(),
            continents: zone::continents(),
            longitude: None,
            longitude_input: String::new(),
            show_map: false,
            map_pick: None,
            show_settings: Arc::new(AtomicBool::new(false)),
            settings: Arc::new(Mutex::new(Settings::default())),
        }
    }

    /// A snapshot of the current settings, read on the main thread each frame.
    fn settings(&self) -> Settings {
        self.settings.lock().map(|g| *g).unwrap_or_default()
    }
}

impl eframe::App for SpyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let cur = self.settings();
        let pal = cur.theme.palette();
        install_theme(ctx, &pal);

        // The native macOS menu's Settings… item opens the settings window.
        if crate::macmenu::settings_selected() {
            self.show_settings.store(true, Ordering::Relaxed);
        }

        let mut dirty = false;
        let text = self
            .latest
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        if text != self.last_text {
            self.last_text.clone_from(&text);
            let new_hits = scan::inspect_text(&text, MAX_READINGS, &self.zone.zone);
            // Only REPLACE the shown reading when the new element actually decodes
            // to something — so moving the cursor across blank / non-timestamp UI
            // (including this panel, which exposes no accessible text) leaves the
            // reading intact instead of wiping it.
            if !new_hits.is_empty() {
                self.source = text;
                self.hits = new_hits;
            }
        }

        // The reference instant for the footer's offset/abbr/DST resolution: the
        // top reading's instant (offset is per-instant), else now.
        let ref_instant = self
            .hits
            .first()
            .and_then(|nr| nr.readings.first())
            .map_or_else(now_instant, |r| r.instant);

        // Footer first (egui requires panels before the central region). The zone
        // control sits at the bottom so the source caption stays prominent.
        egui::TopBottomPanel::bottom("zone_bar")
            .frame(
                Frame::none()
                    .fill(pal.bg_deep)
                    .inner_margin(Margin::symmetric(16.0, 8.0)),
            )
            .show(ctx, |ui| {
                if self.zone_footer(ui, ref_instant) {
                    dirty = true;
                }
            });

        // The clickable world map (floating window), if open.
        if self.map_window(ctx) {
            dirty = true;
        }
        // The settings dialog (bottom-right), if open.
        self.settings_window(ctx);

        // Re-decode when either the hovered text OR the display zone changed.
        if dirty {
            self.hits = scan::inspect_text(&self.source, MAX_READINGS, &self.zone.zone);
        }

        // Snapshot into locals so the nested render closures capture no `self`.
        let source = self.source.clone();
        let hits = std::mem::take(&mut self.hits);
        let zone = self.zone.zone.clone();
        let longitude = self.longitude;
        let show_lunar = cur.show_lunar;

        let panel = Frame::none()
            .fill(pal.bg_deep)
            .inner_margin(Margin::symmetric(16.0, 14.0));
        egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
            header(ui, &source, pal);
            ui.separator();
            ui.add_space(10.0);
            if hits.is_empty() {
                empty_state(
                    ui,
                    "Hover an element with a number",
                    "Point at any on-screen value to decode it",
                    pal,
                );
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for nr in &hits {
                            Frame::none()
                                .fill(pal.bg_card)
                                .rounding(Rounding::same(8.0))
                                .inner_margin(Margin::symmetric(14.0, 12.0))
                                .stroke(Stroke::new(1.0, pal.hairline))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        RichText::new(&nr.number)
                                            .font(FontId::monospace(21.0))
                                            .color(pal.ink)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);
                                    // 2-column grid: the format chip is column 1
                                    // (a tab stop), so every datetime — and the
                                    // 干支 line beneath it — left-aligns in column 2.
                                    egui::Grid::new(nr.number.as_str())
                                        .num_columns(3)
                                        .spacing([10.0, 8.0])
                                        .show(ui, |ui| {
                                            for r in &nr.readings {
                                                conf_cell(ui, r, pal);
                                                chip_cell(ui, r, pal);
                                                datetime_cell(ui, r, &zone, pal);
                                                ui.end_row();
                                                if show_lunar {
                                                    ui.label(""); // col 1 (confidence)
                                                    ui.label(""); // col 2 (format)
                                                    ganzhi_cell(
                                                        ui, r.instant, &zone, longitude, pal,
                                                    );
                                                    ui.end_row();
                                                }
                                            }
                                        });
                                });
                            ui.add_space(10.0);
                        }
                    });
            }
        });

        self.hits = hits;

        // The background poll thread drives repaints when the cursor's element
        // changes; a slow heartbeat keeps the footer's live clock and hover
        // states fresh without busy-spinning the render thread.
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

/// Current wall-clock instant, used as the footer's reference when no reading is
/// on screen yet.
fn now_instant() -> PosixNs {
    PosixNs(jiff::Timestamp::now().as_nanosecond())
}

impl SpyApp {
    /// The footer time-zone control: an active summary (offset · abbr · DST at the
    /// reference instant `at`), UTC/Local presets, a 🌐 map, a Continent → Zone
    /// picker, a ⚙ settings menu (theme, show 干支), and — when 干支 is shown — a
    /// longitude input. Returns `true` when the *zone* changed (settings changes
    /// are purely presentational and need no re-decode).
    fn zone_footer(&mut self, ui: &mut egui::Ui, at: PosixNs) -> bool {
        let pal = self.settings().theme.palette();
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("time zone")
                    .font(FontId::proportional(11.0))
                    .color(pal.faint),
            );
            let (fill, fg) = if self.zone.loud {
                (pal.bg_chip, pal.amber)
            } else {
                (pal.bg_card, pal.mute)
            };
            let summary = zone::zone_summary(&self.zone, at);
            Frame::none()
                .fill(fill)
                .rounding(Rounding::same(4.0))
                .inner_margin(Margin::symmetric(8.0, 3.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(summary)
                            .font(FontId::monospace(12.0))
                            .color(fg)
                            .strong(),
                    );
                });
            ui.add_space(8.0);
            if ui.small_button("UTC").clicked() {
                self.zone = ZoneChoice::default();
                self.map_pick = None;
                changed = true;
            }
            if ui.small_button("Local").clicked() {
                if let Some(z) = parse_zone("local") {
                    self.zone = z;
                    changed = true;
                }
            }
            if ui
                .small_button("🌐")
                .on_hover_text("time-zone map")
                .clicked()
            {
                self.show_map = !self.show_map;
            }
            // One cascading picker: a Region button whose menu lists continents,
            // each a submenu of its zones. egui clips popups to this small
            // always-on-top window, so both levels are wrapped in a height-bounded
            // ScrollArea — a long zone list scrolls instead of being truncated at
            // the window edge. Iterate over a clone so the closure doesn't alias
            // the field it reads.
            let conts = self.continents.clone();
            let max_h = (ui.ctx().screen_rect().height() - 48.0).max(160.0);
            ui.menu_button("Region / Zone…", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(max_h)
                    .show(ui, |ui| {
                        for c in &conts {
                            ui.menu_button(c, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(max_h)
                                    .show(ui, |ui| {
                                        for z in zone::zones_in(c) {
                                            if ui.button(zone::menu_label(&z, at)).clicked() {
                                                if let Some(zc) = parse_zone(&z) {
                                                    self.zone = zc;
                                                    changed = true;
                                                }
                                                ui.close_menu();
                                            }
                                        }
                                    });
                            });
                        }
                    });
            });

            // Global longitude (°E): refines every reading's 干支 hour pillar to
            // true solar time. Only meaningful for 干支, so it is hidden when 干支
            // is off. Optional — empty/invalid means no correction.
            if self.settings().show_lunar {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("longitude")
                        .font(FontId::proportional(11.0))
                        .color(pal.faint),
                );
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.longitude_input)
                        .hint_text("°E")
                        .desired_width(52.0)
                        .font(FontId::monospace(12.0)),
                );
                if resp.changed() {
                    self.longitude = ganzhi::parse_longitude(&self.longitude_input);
                }
            }
        });
        // Selecting a location defaults the 干支 longitude to that zone's central
        // meridian (the user can still override it).
        if changed {
            self.adopt_zone_meridian(at);
        }
        changed
    }

    /// Set the 干支 longitude to `deg` (degrees east), keeping the input buffer in
    /// sync so it round-trips through `parse_longitude`.
    fn set_longitude(&mut self, deg: f64) {
        self.longitude = Some(deg);
        self.longitude_input = format!("{deg}");
    }

    /// Adopt the current zone's central meridian as the 干支 longitude.
    fn adopt_zone_meridian(&mut self, at: PosixNs) {
        if let Some(m) = tzinfo::meridian_longitude(&self.zone.zone, at) {
            self.set_longitude(m);
        }
    }

    /// The settings dialog (theme, whether to show 干支), anchored to the window's
    /// bottom-right. Opened by the footer's ⚙ button; session-scoped, not saved.
    /// The settings dialog as its own floating OS window — a *deferred* egui
    /// viewport (an independent window that honors the builder size, unlike an
    /// immediate viewport), not a panel pinned inside the always-on-top overlay.
    /// Opened from the native macOS menu's Settings… item; its controls write
    /// back through the shared `settings`, and closing it clears `show_settings`.
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings.load(Ordering::Relaxed) {
            return;
        }
        let settings = Arc::clone(&self.settings);
        let open = Arc::clone(&self.show_settings);
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of("settings"),
            egui::ViewportBuilder::default()
                .with_title("timeglyph-spy — Settings")
                .with_inner_size([440.0, 172.0])
                .with_resizable(false),
            move |ctx, _class| {
                let pal = settings
                    .lock()
                    .map(|g| g.theme.palette())
                    .unwrap_or_else(|_| Theme::Dark.palette());
                install_theme(ctx, &pal);
                egui::CentralPanel::default()
                    .frame(
                        Frame::none()
                            .fill(pal.bg_deep)
                            .inner_margin(Margin::same(16.0)),
                    )
                    .show(ctx, |ui| {
                        if let Ok(mut s) = settings.lock() {
                            ui.label(
                                RichText::new("Theme")
                                    .font(FontId::proportional(11.0))
                                    .color(pal.faint),
                            );
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut s.theme, Theme::Dark, "Dark");
                                ui.selectable_value(&mut s.theme, Theme::Light, "Light");
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new("Calendar")
                                    .font(FontId::proportional(11.0))
                                    .color(pal.faint),
                            );
                            ui.checkbox(
                                &mut s.show_lunar,
                                "Chinese lunisolar and heavenly stem / earthly branch",
                            );
                        }
                    });
                if ctx.input(|i| i.viewport().close_requested()) {
                    open.store(false, Ordering::Relaxed);
                }
            },
        );
    }
}

/// Slim header: the wordmark plus a de-emphasised, truncated caption of the raw
/// source element — context, not the subject (and it keeps sensitive surrounding
/// text from dominating the panel).
fn header(ui: &mut egui::Ui, source: &str, pal: Palette) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("◷ timeglyph")
                .font(FontId::monospace(15.0))
                .color(pal.amber)
                .strong(),
        );
        if !source.is_empty() {
            ui.add_space(10.0);
            // Char-safe truncation + single-line Extend. egui 0.29's
            // Label::truncate() byte-slices the galley and PANICS on multi-byte
            // text (e.g. '·'), so we never use it on arbitrary hovered text.
            let collapsed: String = source.split_whitespace().collect::<Vec<_>>().join(" ");
            ui.add(
                egui::Label::new(
                    RichText::new(text::ellipsize(&collapsed, 120))
                        .font(FontId::proportional(11.0))
                        .color(pal.faint),
                )
                .wrap_mode(egui::TextWrapMode::Extend),
            );
        }
    });
}

/// Grid column 1: the amber format chip. The verbose format name is a hover
/// tooltip on the chip (not an always-shown line), keeping each reading compact.
fn chip_cell(ui: &mut egui::Ui, r: &Reading, pal: Palette) {
    Frame::none()
        .fill(pal.bg_chip)
        .rounding(Rounding::same(4.0))
        .inner_margin(Margin::symmetric(6.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(&r.format_id)
                    .font(FontId::monospace(11.0))
                    .color(pal.amber)
                    .strong(),
            );
        })
        .response
        .on_hover_text(r.label.as_str());
}

/// Grid column 2 (row 1): the rendered instant, the per-instant abbreviation and
/// DST (the numeric offset is already in `rendered`; a location alone is
/// ambiguous, so these disambiguate), an optional local tag, and the confidence.
fn datetime_cell(ui: &mut egui::Ui, r: &Reading, zone: &RenderZone, pal: Palette) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&r.rendered)
                .font(FontId::monospace(14.0))
                .color(pal.ink),
        );
        if !r.local {
            if let Some(s) = tzinfo::stamp(zone, r.instant) {
                if !s.abbr.is_empty() {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&s.abbr)
                            .font(FontId::monospace(11.0))
                            .color(pal.mute),
                    );
                }
                if s.dst {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("DST")
                            .font(FontId::proportional(10.0))
                            .color(pal.amber)
                            .strong(),
                    );
                }
            }
        }
        if r.local {
            ui.add_space(6.0);
            ui.label(
                RichText::new("· local (no zone)")
                    .font(FontId::proportional(11.0))
                    .color(pal.faint),
            );
        }
    });
}

/// Grid column 1: the confidence — a red/amber/green dot (by the engine's
/// plausibility score) then the `NN%` — with the named component breakdown on
/// hover. Left of the format chip so the ranking reads at a glance, not just from
/// the row order.
fn conf_cell(ui: &mut egui::Ui, r: &Reading, pal: Palette) {
    let pct = scan::confidence_pct(r.score);
    let dot = if pct >= 67 {
        pal.conf_high
    } else if pct >= 34 {
        pal.conf_mid
    } else {
        pal.conf_low
    };
    let resp = ui
        .horizontal(|ui| {
            ui.label(
                RichText::new("●")
                    .font(FontId::proportional(10.0))
                    .color(dot),
            );
            ui.label(
                RichText::new(format!("{pct}%"))
                    .font(FontId::monospace(11.0))
                    .color(pal.mute),
            );
        })
        .response;
    if !r.components.is_empty() {
        let tip = r
            .components
            .iter()
            .map(|(n, v)| format!("{n}  {v:.2}"))
            .collect::<Vec<_>>()
            .join("\n");
        resp.on_hover_text(format!("plausibility score  {pct}%\n{tip}"));
    }
}

/// Grid column 2 (row 2): the 干支 line, led by the lunar date so it left-aligns
/// under the datetime above it, then the four pillars. Resolved at the display
/// zone (the meridian) and refined by the optional global longitude (hour pillar
/// → true solar time). A reading, not a verdict. Empty cell if the engine can't
/// render (keeps the grid row intact). Shown only when 干支 is enabled.
fn ganzhi_cell(
    ui: &mut egui::Ui,
    instant: PosixNs,
    zone: &RenderZone,
    longitude: Option<f64>,
    pal: Palette,
) {
    let Ok(v) = ganzhi::ganzhi_view(instant, zone, longitude) else {
        ui.label("");
        return;
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{} · {}", v.lunar_date, v.solar_term_phrase()))
                .font(FontId::proportional(10.5))
                .color(pal.faint),
        );
        ui.add_space(8.0);
        for (mark, pillar) in [
            ("年", &v.year_pillar),
            ("月", &v.month_pillar),
            ("日", &v.day_pillar),
            ("時", &v.hour_pillar),
        ] {
            ui.label(
                RichText::new(format!("{mark}{pillar}"))
                    .font(FontId::monospace(11.0))
                    .color(pal.mute),
            );
            ui.add_space(5.0);
        }
    });
}

/// A calm centred placeholder instead of a debug string.
fn empty_state(ui: &mut egui::Ui, title: &str, sub: &str, pal: Palette) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("◷")
                .font(FontId::monospace(34.0))
                .color(pal.glyph),
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new(title)
                .font(FontId::proportional(15.0))
                .color(pal.mute)
                .strong(),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(sub)
                .font(FontId::proportional(12.0))
                .color(pal.faint),
        );
    });
}

impl SpyApp {
    /// The clickable world time-zone map (a floating window). Region *boundaries*
    /// are drawn (Natural Earth, public domain); clicking resolves the point to a
    /// zone via [`tzmap::zone_at`] and sets the display zone — preferring the
    /// region's representative IANA name (DST-aware) over a bare fixed offset.
    /// Returns `true` when the zone changed.
    fn map_window(&mut self, ctx: &egui::Context) -> bool {
        if !self.show_map {
            return false;
        }
        let pal = self.settings().theme.palette();
        let mut changed = false;
        let mut open = true;
        egui::Window::new("time zone map")
            .open(&mut open)
            .default_size([540.0, 300.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "click a region to set the display zone \
                         (Natural Earth · public domain · offset-keyed)",
                    )
                    .font(FontId::proportional(10.5))
                    .color(pal.faint),
                );
                let w = ui.available_width();
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(w, w / 2.0), egui::Sense::click());
                let p = ui.painter_at(rect);
                p.rect_filled(rect, Rounding::same(4.0), pal.bg_deep);
                let proj = |lon: f32, lat: f32| {
                    egui::pos2(
                        rect.left() + (lon + 180.0) / 360.0 * rect.width(),
                        rect.top() + (90.0 - lat) / 180.0 * rect.height(),
                    )
                };
                for r in tzmap::regions() {
                    // Highlight the whole band: every region at the picked offset.
                    let selected = self.map_pick.is_some_and(|o| (o - r.offset).abs() < 0.01);
                    let fill = region_fill(r.offset, selected);
                    let stroke = if selected {
                        Stroke::new(1.4, pal.amber)
                    } else {
                        Stroke::new(0.4, pal.hairline)
                    };
                    for ring in &r.rings {
                        let pts: Vec<egui::Pos2> = ring.iter().map(|c| proj(c[0], c[1])).collect();
                        p.add(egui::Shape::Path(egui::epaint::PathShape {
                            points: pts,
                            closed: true,
                            fill,
                            stroke: stroke.into(),
                        }));
                    }
                }
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let lon = (pos.x - rect.left()) / rect.width() * 360.0 - 180.0;
                        let lat = 90.0 - (pos.y - rect.top()) / rect.height() * 180.0;
                        if let Some(pick) = tzmap::zone_at(lon, lat) {
                            let spec = pick
                                .iana
                                .clone()
                                .unwrap_or_else(|| zone::offset_spec(pick.offset));
                            if let Some(z) = parse_zone(&spec) {
                                self.zone = z;
                                self.map_pick = Some(pick.offset);
                                // The map band offset is already standard time, so
                                // its meridian follows directly.
                                self.set_longitude(tzinfo::meridian_of_offset(pick.offset));
                                changed = true;
                            }
                        }
                    }
                }
            });
        if !open {
            self.show_map = false;
        }
        changed
    }
}

/// A muted fill for a map region, varying by UTC offset so adjacent bands are
/// distinguishable; the selected region fills amber.
fn region_fill(offset: f64, selected: bool) -> Color32 {
    if selected {
        return Color32::from_rgb(214, 158, 46); // amber — the picked band stands out
    }
    // A distinct hue per UTC offset (−12..+14 mapped around most of the colour
    // wheel), muted so the banded continents read like a time-zone map without
    // clashing with the dark panel.
    let t = ((offset + 12.0) / 26.0).clamp(0.0, 1.0);
    hsv(t * 0.82, 0.48, 0.60)
}

/// Minimal HSV→sRGB (`h`, `s`, `v` in `0..1`) for the offset-keyed map bands.
fn hsv(h: f64, s: f64, v: f64) -> Color32 {
    let h6 = (h - h.floor()) * 6.0;
    let i = h6.floor();
    let f = h6 - i;
    let (p, q, w) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as i32 {
        0 => (v, w, p),
        1 => (q, v, p),
        2 => (p, v, w),
        3 => (p, q, v),
        4 => (w, p, v),
        _ => (v, p, q),
    };
    let c = |x: f64| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(c(r), c(g), c(b))
}
