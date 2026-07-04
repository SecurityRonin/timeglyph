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
//! WCAG AA (see [`timeglyph_lens::theme`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use egui::{Color32, FontId, Frame, Margin, RichText, Rounding, Stroke};
use timeglyph::{PosixNs, RenderZone};
use timeglyph_lens::theme::{Palette, Theme};
use timeglyph_lens::zone::{self, parse_zone, ZoneChoice};
use timeglyph_lens::{ganzhi, text, tzinfo, tzmap};

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
pub fn run(verbose: u8) -> Result<(), String> {
    // Fail-fast: verify the picker initializes (e.g. Accessibility permission is
    // granted) before opening the window. The poll thread builds its own picker
    // so the AX handle / COM apartment stays on that thread; this probe is
    // dropped immediately.
    let _ = Picker::new()?;
    // macOS gates the picker behind Accessibility; surface the system prompt on
    // first launch (no-op once granted / on other platforms).
    crate::picker::prompt_accessibility();
    init_tracing(verbose);
    tracing::info!(verbose, "TimeGlyph Lens starting");
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([560.0, 400.0])
        .with_min_inner_size([380.0, 220.0])
        .with_always_on_top()
        .with_title("TimeGlyph Lens");
    // Window / taskbar / dock icon. Falls through silently if it can't decode —
    // a missing icon must not stop the tool opening.
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "timeglyph-lens",
        native_options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            install_theme(&cc.egui_ctx, &Theme::default().palette());
            // Native macOS app menu with a standard Settings… item (⌘,).
            crate::macmenu::install();
            let latest = Arc::new(Mutex::new(String::new()));
            spawn_cursor_poll(cc.egui_ctx.clone(), Arc::clone(&latest));
            Ok(Box::new(LensApp::new(
                latest,
                verbose,
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
            )))
        }),
    )
    .map_err(|e| e.to_string())
}

/// Decode embedded PNG bytes into a texture. `None` if it can't be decoded.
fn load_png_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let icon = eframe::icon_data::from_png_bytes(bytes).ok()?;
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR))
}

/// The app icon texture for the header and empty-state (falls back to the ◷
/// glyph if it can't be decoded).
fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    load_png_texture(ctx, "timeglyph-logo", include_bytes!("../assets/icon.png"))
}

/// Install a stderr tracing subscriber gated by verbosity (`RUST_LOG` overrides):
/// `-v` → info, `-vv` → debug, silent otherwise. Replaces the ad-hoc eprintln
/// verbose dump with structured, filterable events.
fn init_tracing(verbose: u8) {
    use tracing_subscriber::EnvFilter;
    let level = match verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("timeglyph_lens={level},timeglyph={level}")));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
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
    let stack = timeglyph_lens::fonts::fallback_fonts();
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

struct LensApp {
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
    /// Whether the About window is open. Shared with its viewport (flips false on
    /// close). Opened from the macOS About menu item or the clickable corner logo.
    show_about: Arc<AtomicBool>,
    /// Session settings (theme, whether to show 干支). Shared with the settings
    /// viewport so its controls write back to the main window.
    settings: Arc<Mutex<Settings>>,
    /// Verbosity: 0 = quiet; ≥1 logs decoded readings to stderr; ≥2 also shows the
    /// raw element text under the cursor in the panel (a debug caption).
    verbose: u8,
    /// The app logo as a texture, for the header and empty-state. `None` if the
    /// embedded icon can't be decoded — the UI falls back to the ◷ glyph.
    logo: Option<egui::TextureHandle>,
    /// Security Ronin wordmark for the landing screen, in dark- and
    /// light-background variants (picked by the active theme).
    sr_logo_dark: Option<egui::TextureHandle>,
    sr_logo_light: Option<egui::TextureHandle>,
}

impl LensApp {
    /// Drain the native macOS menu: Settings… / About open their windows.
    fn sync_native_menu(&self) {
        let menu = crate::macmenu::selected();
        if menu.settings {
            self.show_settings.store(true, Ordering::Relaxed);
        }
        if menu.about {
            self.show_about.store(true, Ordering::Relaxed);
        }
    }

    /// Pull the latest text the poll thread saw under the cursor and, when it
    /// changed, re-decode it. Only REPLACE the shown reading when the new element
    /// actually decodes to something — so moving the cursor across blank /
    /// non-timestamp UI (including this panel, which exposes no accessible text)
    /// leaves the reading intact instead of wiping it.
    fn ingest_cursor_text(&mut self) {
        let text = self
            .latest
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        if text == self.last_text {
            return;
        }
        self.last_text.clone_from(&text);
        let new_hits = scan::inspect_text(&text, MAX_READINGS, &self.zone.zone);
        if new_hits.is_empty() {
            return;
        }
        self.source = text;
        self.hits = new_hits;
        // Level does the -v/-vv gating: -v → the summary, -vv → the raw element
        // text and every reading.
        tracing::info!(hits = self.hits.len(), "decoded element under cursor");
        tracing::debug!(source = ?self.source, "raw element text");
        for nr in &self.hits {
            for r in &nr.readings {
                tracing::debug!(
                    number = %nr.number,
                    rendered = %r.rendered,
                    format = %r.format_id,
                    "reading"
                );
            }
        }
    }

    /// The theme-matched Security Ronin wordmark in the lower-right corner, tall
    /// enough to span the footer's two rows and anchored so it stays put over any
    /// panel content. Click it for the About dialog, where the version lives — the
    /// main window stays uncluttered.
    fn render_branding(&self, ctx: &egui::Context, sr_logo: Option<&egui::TextureHandle>) {
        let Some(sr) = sr_logo else { return };
        egui::Area::new(egui::Id::new("lens-branding"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -8.0))
            .show(ctx, |ui| {
                let h = 48.0; // native aspect ~1505×721
                let img = egui::Image::new(egui::load::SizedTexture::from_handle(sr))
                    .fit_to_exact_size(egui::vec2(h * 1505.0 / 721.0, h));
                if ui
                    .add(egui::ImageButton::new(img).frame(false))
                    .on_hover_text("About TimeGlyph Lens")
                    .clicked()
                {
                    self.show_about.store(true, Ordering::Relaxed);
                }
            });
    }

    fn new(
        latest: Arc<Mutex<String>>,
        verbose: u8,
        logo: Option<egui::TextureHandle>,
        sr_logo_dark: Option<egui::TextureHandle>,
        sr_logo_light: Option<egui::TextureHandle>,
    ) -> Self {
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
            show_about: Arc::new(AtomicBool::new(false)),
            settings: Arc::new(Mutex::new(Settings::default())),
            verbose,
            logo,
            sr_logo_dark,
            sr_logo_light,
        }
    }

    /// A snapshot of the current settings, read on the main thread each frame.
    fn settings(&self) -> Settings {
        self.settings.lock().map(|g| *g).unwrap_or_default()
    }
}

impl eframe::App for LensApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let cur = self.settings();
        let pal = cur.theme.palette();
        install_theme(ctx, &pal);

        self.sync_native_menu();

        let mut dirty = false;
        self.ingest_cursor_text();

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
        self.about_window(ctx);

        // Re-decode when either the hovered text OR the display zone changed.
        if dirty {
            self.hits = scan::inspect_text(&self.source, MAX_READINGS, &self.zone.zone);
        }

        // Snapshot into locals so the nested render closures capture no `self`.
        // The raw element text is a debug caption — only in -vv.
        let source = if self.verbose >= 2 {
            self.source.clone()
        } else {
            String::new()
        };
        let hits = std::mem::take(&mut self.hits);
        let zone = self.zone.zone.clone();
        let longitude = self.longitude;
        let show_lunar = cur.show_lunar;
        let logo = self.logo.clone();
        let sr_logo = if pal.base_dark {
            self.sr_logo_dark.clone()
        } else {
            self.sr_logo_light.clone()
        };

        let panel = Frame::none()
            .fill(pal.bg_deep)
            .inner_margin(Margin::symmetric(16.0, 14.0));
        egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
            header(ui, &source, pal, logo.as_ref());
            ui.separator();
            ui.add_space(10.0);
            if hits.is_empty() {
                render_empty(ui, pal, logo.as_ref());
            } else {
                render_readings(ui, &hits, &zone, longitude, show_lunar, pal);
            }
        });

        self.render_branding(ctx, sr_logo.as_ref());

        self.hits = hits;

        // The background poll thread drives repaints when the cursor's element
        // changes; a slow heartbeat keeps the footer's live clock and hover
        // states fresh without busy-spinning the render thread.
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

/// The central panel's empty state. On macOS without the Accessibility grant
/// readings never arrive, so prompt for it (with a button that jumps to the
/// pane) instead of sitting on a silent blank.
fn render_empty(ui: &mut egui::Ui, pal: Palette, logo: Option<&egui::TextureHandle>) {
    if crate::picker::accessibility_ok() {
        empty_state(
            ui,
            "Hover an element with a number",
            "Point at any on-screen value to decode it",
            pal,
            logo,
        );
    } else {
        empty_state(
            ui,
            "Grant Accessibility to TimeGlyph Lens",
            "Flip the TimeGlyph Lens switch, then relaunch",
            pal,
            logo,
        );
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            if ui.button("Open Accessibility Settings").clicked() {
                let _ = std::process::Command::new("open")
                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                    .spawn();
            }
        });
    }
}

/// The scrollable list of decoded readings: one card per number, each a
/// confidence / format-chip / datetime grid, with the optional 干支 line beneath.
fn render_readings(
    ui: &mut egui::Ui,
    hits: &[scan::NumberReadings],
    zone: &RenderZone,
    longitude: Option<f64>,
    show_lunar: bool,
    pal: Palette,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for nr in hits {
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
                        // 2-column grid: the format chip is column 1 (a tab stop),
                        // so every datetime — and the 干支 line beneath it —
                        // left-aligns in column 2.
                        egui::Grid::new(nr.number.as_str())
                            .num_columns(3)
                            .spacing([10.0, 8.0])
                            .show(ui, |ui| {
                                for r in &nr.readings {
                                    conf_cell(ui, r, pal);
                                    chip_cell(ui, r, pal);
                                    datetime_cell(ui, r, zone, pal);
                                    ui.end_row();
                                    if show_lunar {
                                        ui.label(""); // col 1 (confidence)
                                        ui.label(""); // col 2 (format)
                                        ganzhi_cell(ui, r.instant, zone, longitude, pal);
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                ui.add_space(10.0);
            }
        });
}

/// Current wall-clock instant, used as the footer's reference when no reading is
/// on screen yet.
fn now_instant() -> PosixNs {
    PosixNs(jiff::Timestamp::now().as_nanosecond())
}

impl LensApp {
    /// The footer time-zone control: an active summary (offset · abbr · DST at the
    /// reference instant `at`), UTC/Local presets, a 🌐 map, a Continent → Zone
    /// picker, a ⚙ settings menu (theme, show 干支), and — when 干支 is shown — a
    /// longitude input. Returns `true` when the *zone* changed (settings changes
    /// are purely presentational and need no re-decode).
    fn zone_footer(&mut self, ui: &mut egui::Ui, at: PosixNs) -> bool {
        let pal = self.settings().theme.palette();
        let mut changed = false;
        // Hide the preset button for the zone that's already active.
        let is_utc = matches!(self.zone.zone, RenderZone::Utc);
        let is_local = self.zone.label == "Local";
        ui.horizontal_wrapped(|ui| {
            // The zone status is always highlighted amber — including UTC — so the
            // active frame is unmistakable at a glance. (No "time zone" caption: the
            // amber chip and the UTC/Local/Region controls make it self-evident.)
            let (fill, fg) = (pal.bg_chip, pal.amber);
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
        });
        // Row 2: the cascading Region → Zone picker, then the 🌐 map toggle to its
        // right. egui clips popups to this small window, so both menu levels use a
        // height-bounded ScrollArea (a long zone list scrolls, not truncated).
        ui.horizontal(|ui| {
            let conts = self.continents.clone();
            let max_h = (ui.ctx().screen_rect().height() - 48.0).max(160.0);
            ui.menu_button("Region / Zone…", |ui| {
                egui::ScrollArea::vertical()
                    .max_height(max_h)
                    .show(ui, |ui| {
                        for c in &conts {
                            ui.menu_button(zone::continent_label(c), |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(max_h)
                                    .show(ui, |ui| {
                                        for (z, label) in zone::menu_entries(c, at) {
                                            if ui.button(label).clicked() {
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
            if ui.button("🌐").on_hover_text("time-zone map").clicked() {
                self.show_map = !self.show_map;
            }
            // In-window settings opener — the only way in on Windows/Linux (the
            // ⌘, native menu item is macOS-only), so theme / 干支 / longitude are
            // reachable on every platform.
            if ui.button("⚙").on_hover_text("settings").clicked() {
                self.show_settings.store(true, Ordering::Relaxed);
            }
            // Quick presets to the right of the map button (each hidden when it's
            // the active zone).
            if !is_utc && ui.button("UTC").clicked() {
                self.zone = ZoneChoice::default();
                self.map_pick = None;
                changed = true;
            }
            if !is_local && ui.button("Local").clicked() {
                if let Some(z) = parse_zone("local") {
                    self.zone = z;
                    changed = true;
                }
            }
            // Global longitude for the 干支 hour-pillar true-solar-time
            // correction — to the right of the presets, shown only with 干支 on;
            // a trailing °E labels the unit. Empty/invalid means no correction.
            if self.settings().show_lunar {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("longitude")
                        .font(FontId::proportional(11.0))
                        .color(pal.faint),
                );
                // Width of exactly 8 monospace digits (enough for e.g. -179.999).
                let box_w = ui.fonts(|f| f.glyph_width(&FontId::monospace(12.0), '0')) * 8.0;
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.longitude_input)
                        .hint_text("120")
                        .desired_width(box_w)
                        .font(FontId::monospace(12.0)),
                );
                if resp.changed() {
                    self.longitude = ganzhi::parse_longitude(&self.longitude_input);
                }
                ui.label(
                    RichText::new("°E")
                        .font(FontId::proportional(11.0))
                        .color(pal.faint),
                );
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
                .with_title("TimeGlyph Lens — Settings")
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

    /// The About dialog: a deferred viewport showing the theme-matched Security
    /// Ronin logo and the version. Opened from the macOS About item or the
    /// clickable corner logo; closing clears `show_about`.
    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about.load(Ordering::Relaxed) {
            return;
        }
        let settings = Arc::clone(&self.settings);
        let open = Arc::clone(&self.show_about);
        let sr_dark = self.sr_logo_dark.clone();
        let sr_light = self.sr_logo_light.clone();
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of("about"),
            egui::ViewportBuilder::default()
                .with_title("About TimeGlyph Lens")
                .with_inner_size([360.0, 260.0])
                .with_resizable(false),
            move |ctx, _class| {
                let pal = settings
                    .lock()
                    .map(|g| g.theme.palette())
                    .unwrap_or_else(|_| Theme::Dark.palette());
                install_theme(ctx, &pal);
                let sr = if pal.base_dark {
                    sr_dark.as_ref()
                } else {
                    sr_light.as_ref()
                };
                egui::CentralPanel::default()
                    .frame(
                        Frame::none()
                            .fill(pal.bg_deep)
                            .inner_margin(Margin::same(20.0)),
                    )
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(6.0);
                            if let Some(tex) = sr {
                                let h = 96.0; // native aspect ~1505×721
                                ui.add(
                                    egui::Image::new(egui::load::SizedTexture::from_handle(tex))
                                        .fit_to_exact_size(egui::vec2(h * 1505.0 / 721.0, h)),
                                );
                            }
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("TimeGlyph Lens")
                                    .font(FontId::monospace(16.0))
                                    .color(pal.ink)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("TimeGlyph {}", timeglyph::VERSION))
                                    .font(FontId::proportional(12.0))
                                    .color(pal.mute),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("© 2026 Security Ronin Ltd")
                                    .font(FontId::proportional(11.0))
                                    .color(pal.faint),
                            );
                        });
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
fn header(ui: &mut egui::Ui, source: &str, pal: Palette, logo: Option<&egui::TextureHandle>) {
    ui.horizontal(|ui| {
        if let Some(tex) = logo {
            ui.add(
                egui::Image::new(egui::load::SizedTexture::from_handle(tex))
                    .fit_to_exact_size(egui::vec2(20.0, 20.0)),
            );
            ui.add_space(2.0);
        }
        ui.label(
            RichText::new(if logo.is_some() {
                "TimeGlyph"
            } else {
                "◷ TimeGlyph"
            })
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
    ui.horizontal(|ui| {
        // Uniform row height (see `row_h`) so the format chip centers on the same
        // midline as every other cell and the dots stay evenly spaced.
        ui.set_min_height(row_h(ui));
        Frame::none()
            .fill(pal.bg_chip)
            .rounding(Rounding::same(4.0))
            .inner_margin(Margin::symmetric(6.0, 2.0))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(&r.format_id)
                            .font(FontId::monospace(11.0))
                            .color(pal.amber)
                            .strong(),
                    )
                    .wrap_mode(egui::TextWrapMode::Extend),
                );
            })
            .response
            .on_hover_text(r.label.as_str());
    });
}

/// The uniform height of *every* reading row — the two-line "local time /
/// (not time-zone adjusted)" tag's height (its two lines' font row heights,
/// stacked with zero inter-line spacing), which is the tallest a row gets.
/// Pinning all three cells of all rows to this one height is what makes the
/// confidence dots evenly spaced: dot spacing = row height + grid spacing, so it
/// is constant only when every row is the same height. Single-line rows gain a
/// little breathing room (content centered in the taller row); the local row's
/// tag fills it exactly, its inter-line gap landing on the shared midline.
fn row_h(ui: &egui::Ui) -> f32 {
    ui.fonts(|f| {
        f.row_height(&FontId::proportional(11.0)) + f.row_height(&FontId::proportional(10.0))
    })
}

fn datetime_cell(ui: &mut egui::Ui, r: &Reading, zone: &RenderZone, pal: Palette) {
    let datetime = || {
        RichText::new(&r.rendered)
            .font(FontId::monospace(14.0))
            .color(pal.ink)
    };
    ui.horizontal(|ui| {
        // Uniform row height (see `row_h`) across all rows so dots stay evenly
        // spaced; egui's Align::Center then centers each cell's content on the
        // shared midline.
        ui.set_min_height(row_h(ui));
        if r.local {
            ui.label(datetime());
            ui.add_space(6.0);
            ui.vertical(|ui| {
                // Tighten the two tag lines: the vertical inherits the grid's
                // 8px row spacing, which reads as a gap between them.
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(
                    RichText::new("local time")
                        .font(FontId::proportional(11.0))
                        .color(pal.faint),
                );
                ui.label(
                    RichText::new("(not time-zone adjusted)")
                        .font(FontId::proportional(10.0))
                        .color(pal.faint),
                );
            });
            if let Some(wd) = scan::weekday(&r.rendered) {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(wd)
                        .font(FontId::proportional(11.0))
                        .color(pal.faint),
                );
            }
            return;
        }
        ui.label(datetime());
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
                    RichText::new("☀ DST")
                        .font(FontId::proportional(10.0))
                        .color(pal.amber)
                        .strong(),
                );
            }
        } else if r.rendered.ends_with('Z') {
            // UTC display zone: show a "UTC" designator like a named zone shows
            // its abbreviation, for consistency — even though the value's own
            // `Z` already says UTC.
            ui.add_space(6.0);
            ui.label(
                RichText::new("UTC")
                    .font(FontId::monospace(11.0))
                    .color(pal.mute),
            );
        }
        // Weekday of the displayed date — handy for spotting what day an event
        // fell on.
        if let Some(wd) = scan::weekday(&r.rendered) {
            ui.add_space(6.0);
            ui.label(
                RichText::new(wd)
                    .font(FontId::proportional(11.0))
                    .color(pal.faint),
            );
        }
        // Public holiday for this date in the display zone — only a named IANA
        // zone maps to a country. An annotation ("consistent with a public
        // holiday there"), in the country's own locale; not proof it was observed.
        if let Some(name) = timeglyph::holiday::in_zone_rendered(zone, &r.rendered) {
            ui.add_space(6.0);
            ui.label(
                RichText::new(name)
                    .font(FontId::proportional(11.0))
                    .color(pal.amber)
                    .strong(),
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
            // Uniform row height (see `row_h`) so the dot centers on the shared
            // midline and the dots stay evenly spaced down the list.
            ui.set_min_height(row_h(ui));
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

/// Spot colour for a 干支 character by its 五行 (Five Element): Wood→green,
/// Fire→red, Earth→ochre, Metal→pale gold, Water→blue. Tuned for the dark theme;
/// a non-干支 char (never expected inside a pillar) falls back to `pal.mute`.
fn element_color(ch: char, pal: Palette) -> Color32 {
    use timeglyph_lens::ganzhi::Element;
    match ganzhi::five_element(ch) {
        Some(Element::Wood) => Color32::from_rgb(0x3f, 0xbf, 0x6a),
        Some(Element::Fire) => Color32::from_rgb(0xec, 0x5b, 0x4d),
        Some(Element::Earth) => Color32::from_rgb(0xd9, 0x9a, 0x2b),
        Some(Element::Metal) => Color32::from_rgb(0xd9, 0xd2, 0xb0),
        Some(Element::Water) => Color32::from_rgb(0x46, 0xa3, 0xe6),
        None => pal.mute,
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
    // horizontal_top (Align::Min) so the lunar date string and the stem (top)
    // row share the same top line — and, since every pillar stack top-aligns
    // too, the four stems land on one row and the four branches on the next.
    ui.horizontal_top(|ui| {
        ui.label(
            RichText::new(format!("{} · {}", v.lunar_date, v.solar_term_phrase()))
                .font(FontId::proportional(10.5))
                .color(pal.faint),
        );
        ui.add_space(8.0);
        for (unit, pillar) in [
            ("年", v.year_pillar.as_str()),
            ("月", v.month_pillar.as_str()),
            ("日", v.day_pillar.as_str()),
            ("時", v.hour_pillar.as_str()),
        ] {
            // Stem (天干) over branch (地支), each spot-coloured by its 五行; the
            // unit character kept as a faint suffix to the right of the stack.
            let stem = pillar.chars().next().unwrap_or(' ');
            let branch = pillar.chars().nth(1).unwrap_or(' ');
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(
                        RichText::new(stem.to_string())
                            .font(FontId::monospace(13.0))
                            .color(element_color(stem, pal)),
                    );
                    let branch_resp = ui.label(
                        RichText::new(branch.to_string())
                            .font(FontId::monospace(13.0))
                            .color(element_color(branch, pal)),
                    );
                    // Ring the day branch (日支) — the anchor pillar of the chart.
                    if unit == "日" {
                        let r = branch_resp.rect;
                        ui.painter().circle_stroke(
                            r.center(),
                            r.width().max(r.height()) * 0.5 + 1.0,
                            Stroke::new(1.3, pal.ink),
                        );
                    }
                });
                ui.label(
                    RichText::new(unit)
                        .font(FontId::proportional(9.0))
                        .color(pal.faint),
                );
            });
            ui.add_space(6.0);
        }
    });
}

/// A calm centred placeholder instead of a debug string.
fn empty_state(
    ui: &mut egui::Ui,
    title: &str,
    sub: &str,
    pal: Palette,
    logo: Option<&egui::TextureHandle>,
) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        if let Some(tex) = logo {
            ui.add(
                egui::Image::new(egui::load::SizedTexture::from_handle(tex))
                    .fit_to_exact_size(egui::vec2(72.0, 72.0)),
            );
        } else {
            ui.label(
                RichText::new("◷")
                    .font(FontId::monospace(34.0))
                    .color(pal.glyph),
            );
        }
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

impl LensApp {
    /// The clickable world time-zone map (a floating window). Region *boundaries*
    /// are drawn (Natural Earth, public domain); clicking resolves the point to a
    /// zone via [`tzmap::zone_at`] and sets the display zone — preferring the
    /// region's representative IANA name (DST-aware) over a bare fixed offset.
    /// Returns `true` when the zone changed.
    fn map_window(&mut self, ctx: &egui::Context) -> bool {
        if !self.show_map {
            return false;
        }
        // Dismiss like a popup selection box: Escape closes it (clicking a band
        // closes it too — see below; and the 🌐 button toggles it).
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_map = false;
        }
        let pal = self.settings().theme.palette();
        let mut changed = false;
        egui::Window::new("time zone map")
            .title_bar(false)
            .resizable(false)
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
                // Coastline overlay: outline the continents over the offset bands
                // so land reads distinctly from ocean. Stroke only — filling these
                // very concave polygons makes egui's tessellator spike.
                for ring in tzmap::land() {
                    let pts: Vec<egui::Pos2> = ring.iter().map(|c| proj(c[0], c[1])).collect();
                    p.add(egui::Shape::Path(egui::epaint::PathShape {
                        points: pts,
                        closed: true,
                        fill: Color32::TRANSPARENT,
                        stroke: Stroke::new(1.0, Color32::from_rgb(238, 235, 228)).into(),
                    }));
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
                                // Selection box: close the map as soon as a band
                                // is picked.
                                self.show_map = false;
                            }
                        }
                    }
                }
            });
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
