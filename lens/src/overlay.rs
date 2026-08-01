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
use egui::{Color32, CornerRadius, FontId, Frame, Margin, RichText, Stroke};
use timeglyph::{DateStyle, PosixNs, RenderZone};
use timeglyph_lens::clipboard::{
    self, ClipboardOutcome, ClipboardRead, ClipboardUnavailable, SourceContext, SystemClipboard,
};
use timeglyph_lens::settings as persist;
use timeglyph_lens::theme::{Palette, Theme, ThemePreference};
use timeglyph_lens::zone::{self, parse_zone, ZoneChoice};
use timeglyph_lens::{altcal, ganzhi, text, tzinfo, tzmap};

use crate::picker::Picker;
use crate::scan::{self, NumberReadings, Reading};

/// The live overlay settings (theme, 干支 line, datetime display style). Loaded
/// from and saved to disk (see [`timeglyph_lens::settings`]) so a prior
/// session's display frame carries over.
#[derive(Clone, Copy, Default)]
struct Settings {
    /// The theme preference (System / Dark / Light). `System` follows the OS.
    theme_pref: ThemePreference,
    /// The concrete palette theme, re-resolved from `theme_pref` + the OS setting
    /// each frame (see `update`), so all palette lookups read a plain [`Theme`].
    theme: Theme,
    /// Whether to show the 干支 / lunisolar line (and, with it, the longitude
    /// input, which only refines the 干支 hour pillar). Off by default.
    show_lunar: bool,
    /// Which alternative calendars are shown in the calendar expansion.
    calendars: persist::CalendarVisibility,
    /// Datetime display style for rendered readings (ISO 8601 by default).
    date_style: DateStyle,
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
        // Wide enough that the alternative-calendar rows don't wrap; a compact
        // height (the settings panel scrolls if it needs more).
        .with_inner_size([680.0, 420.0])
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
                // Load persisted preferences (a missing/corrupt file degrades to
                // defaults). Injected here rather than inside `new` so the render
                // gate can pass a fixed, hermetic settings snapshot instead of
                // reading the host's config.
                persist::load(),
                // The real platform clipboard; a failure to open is carried as the
                // button's disabled reason rather than a startup abort.
                SystemClipboard::new().map(|c| Box::new(c) as Box<dyn ClipboardRead>),
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
            .insert(key.to_owned(), egui::FontData::from_owned(bytes).into());
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
    // `style_mut` was removed in egui 0.35 (styles are now per-theme). Apply the
    // spacing to both themes so it holds whichever theme egui renders with,
    // matching the old single global style.
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |s| s.spacing.item_spacing = egui::vec2(8.0, 6.0));
    }
}

/// Map egui's reported OS theme to the overlay's own [`Theme`].
fn from_egui_theme(t: egui::Theme) -> Theme {
    match t {
        egui::Theme::Dark => Theme::Dark,
        egui::Theme::Light => Theme::Light,
    }
}

struct LensApp {
    /// Latest text under the cursor, produced by the background poll thread; the
    /// render thread only reads this snapshot (never the AX/UIA API directly).
    latest: Arc<Mutex<String>>,
    last_text: String,
    /// Where the decoded text came from. A [`SourceContext`] rather than a bare
    /// `String` so what may be captioned is decided by the type: the cursor
    /// variant carries its element text, and any future non-cursor source (the
    /// clipboard) structurally cannot put content on screen.
    source: SourceContext,
    /// The decoded model: numbers and their ranked readings.
    hits: Vec<NumberReadings>,
    /// A fixed note from the last clipboard press that produced no readings, shown
    /// until the next press or hover replaces the display.
    ///
    /// A miss deliberately leaves `hits` alone so a misfired press cannot wipe the
    /// reading being studied — but silence would leave those readings looking like
    /// the press's answer, so the miss says so instead. Always a constant from
    /// [`ClipboardOutcome::notice`], never clipboard content.
    clipboard_notice: Option<&'static str>,
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
    /// Freeze toggle: when `true`, `ingest_cursor_text` is skipped so the user
    /// can hover over the overlay to read or copy without the displayed reading
    /// changing. Toggled by the ⏸ header button or the Space key.
    frozen: Arc<AtomicBool>,
    /// Session settings (theme, whether to show 干支). Shared with the settings
    /// viewport so its controls write back to the main window.
    settings: Arc<Mutex<Settings>>,
    /// The platform clipboard, for the 🗐 control — or why it is unavailable, which
    /// the disabled button shows as its reason. Opened once at startup and read
    /// only when pressed; nothing polls it.
    /// Injected, never built here: the button's enabled/disabled state changes the
    /// rendered frame, so constructing the platform clipboard inside `new` made the
    /// offscreen render gate depend on whether the HOST had a usable pasteboard —
    /// green on a dev Mac, red on a headless runner. Boxed behind the trait so the
    /// gate can supply a deterministic one (the same reason `saved` is injected).
    clipboard: Result<Box<dyn ClipboardRead>, ClipboardUnavailable>,
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
        // Frozen: hold the current reading so the cursor can move onto the overlay
        // to read/copy without the hovered value changing.
        if self.frozen.load(Ordering::Relaxed) {
            return;
        }
        let text = self
            .latest
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        if text == self.last_text {
            return;
        }
        self.last_text.clone_from(&text);
        let new_hits = scan::inspect_text(&text, timeglyph_lens::READINGS_SHOWN, &self.zone.zone);
        if new_hits.is_empty() {
            return;
        }
        self.source = SourceContext::Cursor(text);
        self.hits = new_hits;
        // A hover result supersedes the last clipboard miss: the note explained why
        // *those* readings were not from the press, and these are new readings.
        self.clipboard_notice = None;
        // Level does the -v/-vv gating: -v → the summary, -vv → the raw element
        // text and every reading.
        tracing::info!(hits = self.hits.len(), "decoded element under cursor");
        match &self.source {
            SourceContext::Cursor(text) => tracing::debug!(source = ?text, "raw element text"),
            // Never log content for a source whose text must not be displayed —
            // a log line is as readable as the caption. Kind and size only.
            SourceContext::Clipboard => {
                tracing::debug!(source = "clipboard", "decoded a non-cursor source");
            }
        }
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

    /// Decode the clipboard once, because the user pressed 🗐.
    ///
    /// The way in for a value hovering cannot reach: the host accessibility tree
    /// stops at a VM guest window, but the clipboard crosses that boundary. One
    /// read per press — no thread, no watcher, no polling.
    ///
    /// A clipboard holding nothing decodable leaves the readings untouched, so a
    /// misfired press cannot wipe the reading being read — but it reports the miss,
    /// because silence would leave those readings looking like the press's answer.
    fn decode_clipboard_now(&mut self) {
        // Cloned first: the decode borrows the clipboard mutably.
        let zone = self.zone.zone.clone();
        let Ok(clipboard) = &mut self.clipboard else {
            // The button is disabled without a clipboard, so a press cannot reach
            // here; the reason was named at startup.
            return;
        };
        match clipboard::decode(&mut **clipboard, timeglyph_lens::READINGS_SHOWN, &zone) {
            ClipboardOutcome::Decoded(source, hits) => {
                self.source = source;
                self.hits = hits;
                self.clipboard_notice = None;
                tracing::info!(hits = self.hits.len(), "decoded the clipboard");
            }
            miss => {
                // Keep the readings; say that these are not what the press found.
                self.clipboard_notice = miss.notice();
                tracing::info!(outcome = ?miss, "the clipboard held nothing decodable");
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
                    .add(egui::Button::image(img).frame(false))
                    .on_hover_text("About TimeGlyph Lens")
                    .clicked()
                {
                    // A second click on the logo closes the About window (toggle).
                    let open = self.show_about.load(Ordering::Relaxed);
                    self.show_about.store(!open, Ordering::Relaxed);
                }
            });
    }

    fn new(
        latest: Arc<Mutex<String>>,
        verbose: u8,
        logo: Option<egui::TextureHandle>,
        sr_logo_dark: Option<egui::TextureHandle>,
        sr_logo_light: Option<egui::TextureHandle>,
        saved: persist::PersistedSettings,
        clipboard: Result<Box<dyn ClipboardRead>, ClipboardUnavailable>,
    ) -> Self {
        let zone = parse_zone(&saved.zone_spec).unwrap_or_default();
        let longitude_input = saved.longitude.map(|d| format!("{d}")).unwrap_or_default();
        Self {
            latest,
            last_text: String::new(),
            source: SourceContext::Cursor(String::new()),
            hits: Vec::new(),
            clipboard_notice: None,
            zone,
            continents: zone::continents(),
            longitude: saved.longitude,
            longitude_input,
            show_map: false,
            map_pick: None,
            show_settings: Arc::new(AtomicBool::new(false)),
            show_about: Arc::new(AtomicBool::new(false)),
            frozen: Arc::new(AtomicBool::new(false)),
            settings: Arc::new(Mutex::new(Settings {
                theme_pref: saved.theme,
                // Concrete placeholder; update() re-resolves it from theme_pref +
                // the OS setting each frame.
                theme: saved.theme.resolve(None),
                show_lunar: saved.show_lunar,
                calendars: saved.calendars,
                date_style: saved.date_style,
            })),
            clipboard,
            verbose,
            logo,
            sr_logo_dark,
            sr_logo_light,
        }
    }

    /// Snapshot the current live state into a [`persist::PersistedSettings`] and
    /// write it to disk. Called whenever a setting or the display zone changes.
    fn save_settings(&self) {
        let cur = self.settings();
        persist::save(&persist::PersistedSettings {
            theme: cur.theme_pref,
            show_lunar: cur.show_lunar,
            date_style: cur.date_style,
            zone_spec: self.zone_spec(),
            longitude: self.longitude,
            calendars: cur.calendars,
        });
    }

    /// The current display zone as a `parse_zone` spec string (round-trips
    /// through [`parse_zone`] on the next launch).
    fn zone_spec(&self) -> String {
        match &self.zone.zone {
            RenderZone::Utc => "UTC".to_string(),
            // `Local` is a resolved system zone; persist the intent, not the
            // resolved name, so it re-resolves on a machine in a different zone.
            _ if self.zone.label == "Local" => "local".to_string(),
            RenderZone::Named(tz) => tz
                .iana_name()
                .map_or_else(|| self.zone.label.clone(), str::to_string),
            RenderZone::Fixed(_) => self.zone.label.clone(),
        }
    }

    /// A snapshot of the current settings, read on the main thread each frame.
    fn settings(&self) -> Settings {
        self.settings.lock().map(|g| *g).unwrap_or_default()
    }
}

impl eframe::App for LensApp {
    // egui 0.35 replaced `App::update(ctx, frame)` with a required
    // `ui(&mut self, ui, frame)` (plus a provided `logic`). The given `ui` covers
    // the full central area with no margin/background, so the app's own panels are
    // shown into it with `show_inside` — layout-equivalent to the old top-level
    // `show(ctx, …)`. `ctx` is cloned (cheap Arc) to keep it usable while `ui` is
    // borrowed mutably by the panels.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Resolve the concrete palette theme from the preference + the OS setting
        // each frame: `System` follows the OS live; `Dark`/`Light` are fixed.
        let system = ctx.system_theme().map(from_egui_theme);
        if let Ok(mut s) = self.settings.lock() {
            s.theme = s.theme_pref.resolve(system);
        }
        let cur = self.settings();
        let pal = cur.theme.palette();
        install_theme(&ctx, &pal);

        self.sync_native_menu();

        // Space toggles the freeze/pin state (the ⏸/▶ header button is the other
        // path). Read before ingest so a press takes effect this frame.
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            let f = self.frozen.load(Ordering::Relaxed);
            self.frozen.store(!f, Ordering::Relaxed);
        }

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
        egui::containers::Panel::bottom("zone_bar")
            .frame(
                Frame::NONE
                    .fill(pal.bg_deep)
                    .inner_margin(Margin::symmetric(16, 8)),
            )
            .show(ui, |ui| {
                if self.zone_footer(ui, ref_instant) {
                    dirty = true;
                }
            });

        // The clickable world map (floating window), if open.
        if self.map_window(&ctx) {
            dirty = true;
        }
        // The settings dialog (top-right), if open.
        self.settings_window(&ctx);
        self.about_window(&ctx);

        // Re-decode when either the hovered text OR the display zone changed.
        if dirty {
            // A cursor source keeps its element text, so the zone change re-renders
            // from it. A source that retains no text (the clipboard) has nothing to
            // re-decode here — such a path re-reads its origin instead.
            let redecoded = match &self.source {
                SourceContext::Cursor(text) => Some(scan::inspect_text(
                    text,
                    timeglyph_lens::READINGS_SHOWN,
                    &self.zone.zone,
                )),
                SourceContext::Clipboard => None,
            };
            if let Some(hits) = redecoded {
                self.hits = hits;
            }
        }

        // Snapshot into locals so the nested render closures capture no `self`.
        // The source caption is a debug aid — only in -vv, and only for a source
        // whose text is displayable at all.
        let source = if self.verbose >= 2 {
            timeglyph_lens::clipboard::caption(&self.source)
        } else {
            None
        };
        let hits = std::mem::take(&mut self.hits);
        // Shown in every mode, unlike the -vv source caption: a press that found
        // nothing has to be visible, or the readings still on screen read as its
        // answer. `Option<&'static str>` is `Copy`, and never clipboard content.
        let clipboard_notice = self.clipboard_notice;
        let zone = self.zone.zone.clone();
        let longitude = self.longitude;
        let show_lunar = cur.show_lunar;
        let cals = cur.calendars;
        let date_style = cur.date_style;
        let logo = self.logo.clone();
        // Cloned Arcs so the header's top-right controls (gear, ⏸ freeze) can
        // write back without the central closure borrowing `self`.
        let show_settings = self.show_settings.clone();
        let frozen = self.frozen.clone();
        // Why 🗐 is disabled, if it is. The reason travels to its tooltip so an
        // unavailable clipboard reads as that, and not as an empty one.
        let no_clipboard = self.clipboard.as_ref().err().map(ToString::to_string);
        let sr_logo = if pal.base_dark {
            self.sr_logo_dark.clone()
        } else {
            self.sr_logo_light.clone()
        };

        let mut decode_clipboard = false;
        let panel = Frame::NONE
            .fill(pal.bg_deep)
            .inner_margin(Margin::symmetric(16, 14));
        egui::CentralPanel::default().frame(panel).show(ui, |ui| {
            decode_clipboard = header(
                ui,
                source.as_deref(),
                pal,
                logo.as_ref(),
                &show_settings,
                &frozen,
                no_clipboard.as_deref(),
            );
            ui.separator();
            ui.add_space(10.0);
            if let Some(note) = clipboard_notice {
                // Deliberately directly above the readings, because it is those
                // readings it disclaims: the press found nothing, so whatever is
                // below did not come from it.
                ui.label(egui::RichText::new(note).color(pal.mute).italics());
                ui.add_space(6.0);
            }
            if hits.is_empty() {
                render_empty(ui, pal, logo.as_ref());
            } else {
                render_readings(
                    ui,
                    &hits,
                    &zone,
                    longitude,
                    CalPrefs {
                        show_lunar,
                        calendars: cals,
                    },
                    date_style,
                    pal,
                );
            }
        });

        self.render_branding(&ctx, sr_logo.as_ref());

        self.hits = hits;

        // After the readings are back in place, so a successful clipboard decode
        // replaces them (and a fruitless one leaves them alone).
        if decode_clipboard {
            self.decode_clipboard_now();
        }

        // The background poll thread drives repaints when the cursor's element
        // changes; a slow heartbeat keeps the footer's live clock and hover
        // states fresh without busy-spinning the render thread.
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

/// The author's profile — the Security Ronin wordmark links here.
const AUTHOR_URL: &str = "https://www.linkedin.com/in/alberthui";

/// Open a URL in the user's default browser, cross-platform (the lens ships on
/// macOS and Windows; the third arm covers the in-progress Linux build).
fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        // `start` is a cmd builtin; the empty "" is the window title, so the URL
        // isn't misread as one.
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
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

/// The calendar-expansion preferences: whether the 干支 expansion shows, and which
/// alternative calendars within it are enabled.
#[derive(Clone, Copy)]
struct CalPrefs {
    show_lunar: bool,
    calendars: persist::CalendarVisibility,
}

/// The scrollable list of decoded readings: one card per number, each a
/// confidence / format-chip / datetime grid, with the optional 干支 line beneath.
fn render_readings(
    ui: &mut egui::Ui,
    hits: &[scan::NumberReadings],
    zone: &RenderZone,
    longitude: Option<f64>,
    cal: CalPrefs,
    date_style: DateStyle,
    pal: Palette,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for nr in hits {
                Frame::NONE
                    .fill(pal.bg_card)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(14, 12))
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
                                    datetime_cell(ui, r, zone, date_style, pal);
                                    ui.end_row();
                                    // The 干支 row and the alternative-calendar row
                                    // are independent — each drawn on its own toggle.
                                    if cal.show_lunar {
                                        ui.label(""); // col 1 (confidence)
                                        ui.label(""); // col 2 (format)
                                        ganzhi_cell(ui, r.instant, zone, longitude, pal);
                                        ui.end_row();
                                    }
                                    if cal.calendars.any() {
                                        ui.label(""); // col 1
                                        ui.label(""); // col 2
                                        altcal_cell(ui, r.instant, zone, cal.calendars, pal);
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
        ui.horizontal_wrapped(|ui| {
            // The zone status is always highlighted amber — including UTC — so the
            // active frame is unmistakable at a glance. (No "time zone" caption: the
            // amber chip and the UTC/Local/Region controls make it self-evident.)
            let (fill, fg) = (pal.bg_chip, pal.amber);
            let summary = zone::zone_summary(&self.zone, at);
            Frame::NONE
                .fill(fill)
                .corner_radius(CornerRadius::same(4))
                .inner_margin(Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(summary)
                            .font(FontId::monospace(12.0))
                            .color(fg)
                            .strong(),
                    );
                });
        });
        self.zone_controls(ui, at)
    }

    /// The shared display-timezone controls: the cascading Region → Zone picker,
    /// the 🌐 world-map toggle, a ⚙ settings opener, the UTC / Local presets, and
    /// (with 干支 on) the longitude input. Rendered identically in the main-window
    /// footer and the Settings dialog. Returns `true` when the *zone* changed.
    ///
    /// egui clips popups to the small window, so both menu levels use a
    /// height-bounded ScrollArea (a long zone list scrolls, not truncated).
    fn zone_controls(&mut self, ui: &mut egui::Ui, at: PosixNs) -> bool {
        let pal = self.settings().theme.palette();
        let mut changed = false;
        // Hide the preset button for the zone that's already active.
        let is_utc = matches!(self.zone.zone, RenderZone::Utc);
        let is_local = self.zone.label == "Local";
        ui.horizontal(|ui| {
            let conts = self.continents.clone();
            let max_h = (ui.ctx().content_rect().height() - 48.0).max(160.0);
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
                                                ui.close();
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
                let box_w = ui.fonts_mut(|f| f.glyph_width(&FontId::monospace(12.0), '0')) * 8.0;
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.longitude_input)
                        .hint_text("120")
                        .desired_width(box_w)
                        .font(FontId::monospace(12.0)),
                );
                if resp.changed() {
                    self.longitude = ganzhi::parse_longitude(&self.longitude_input);
                    // A manual longitude edit is a persisted preference too.
                    self.save_settings();
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
            self.save_settings();
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

    /// The settings dialog: theme, the datetime display style, and the per-calendar
    /// toggles. Opened by the footer's ⚙ button or the native macOS Settings… item;
    /// the display timezone / longitude live in the footer, not here. Closed by the
    /// ✕, a second ⚙ click, or Escape. A change writes back through `settings` and
    /// is persisted to disk.
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings.load(Ordering::Relaxed) {
            return;
        }
        let pal = self.settings().theme.palette();
        let mut open = true;
        let mut settings_changed = false;
        // Cap the panel to the viewport so tall content (seven calendar toggles +
        // the zone / longitude section) scrolls instead of clipping the bottom.
        let max_h = (ctx.content_rect().height() - 90.0).max(180.0);
        // Anchor to the TOP so the title bar (with egui's ✕) is always on-screen —
        // a bottom anchor pushes the title bar off the top edge once the content
        // is tall (the six calendar toggles).
        egui::Window::new("TimeGlyph Lens — Settings")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
            .frame(
                // A raised surface (bg_card) with a hairline border, so the panel
                // reads as a distinct floating pane over the main window rather
                // than blending into the same bg_deep background.
                Frame::NONE
                    .fill(pal.bg_card)
                    .stroke(egui::Stroke::new(1.0, pal.hairline))
                    .inner_margin(Margin::same(16)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .max_height(max_h)
                    .show(ui, |ui| {
                        if let Ok(mut s) = self.settings.lock() {
                            ui.label(
                                RichText::new("Theme")
                                    .font(FontId::proportional(11.0))
                                    .color(pal.faint),
                            );
                            ui.horizontal(|ui| {
                                settings_changed |= ui
                                    .selectable_value(
                                        &mut s.theme_pref,
                                        ThemePreference::System,
                                        "System",
                                    )
                                    .changed();
                                settings_changed |= ui
                                    .selectable_value(
                                        &mut s.theme_pref,
                                        ThemePreference::Light,
                                        "Light",
                                    )
                                    .changed();
                                settings_changed |= ui
                                    .selectable_value(
                                        &mut s.theme_pref,
                                        ThemePreference::Dark,
                                        "Dark",
                                    )
                                    .changed();
                            });
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new("Date format")
                                    .font(FontId::proportional(11.0))
                                    .color(pal.faint),
                            );
                            let style_label = |st: DateStyle| match st {
                                DateStyle::Iso8601 => "ISO 8601",
                                DateStyle::SpaceSeparated => "Space-separated",
                                DateStyle::Rfc2822 => "RFC 2822",
                                DateStyle::UsStyle => "US (12-hour)",
                            };
                            egui::ComboBox::from_id_salt("date_style")
                                .selected_text(style_label(s.date_style))
                                .show_ui(ui, |ui| {
                                    for st in [
                                        DateStyle::Iso8601,
                                        DateStyle::SpaceSeparated,
                                        DateStyle::Rfc2822,
                                        DateStyle::UsStyle,
                                    ] {
                                        settings_changed |= ui
                                            .selectable_value(
                                                &mut s.date_style,
                                                st,
                                                style_label(st),
                                            )
                                            .changed();
                                    }
                                });
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new("Calendar")
                                    .font(FontId::proportional(11.0))
                                    .color(pal.faint),
                            );
                            // Every calendar is an independent toggle — the 干支/lunisolar
                            // line and the six alternative calendars are siblings, not a
                            // hierarchy (the alt calendars are not part of the 干支 view).
                            settings_changed |= ui
                                .checkbox(&mut s.show_lunar, "農曆+干支暦 Lunisolar + Stem-Branch")
                                .changed();
                            let c = &mut s.calendars;
                            settings_changed |= ui
                                .checkbox(&mut c.roc, "中華民國 Republic of China")
                                .changed();
                            settings_changed |=
                                ui.checkbox(&mut c.japanese, "和暦 Japanese").changed();
                            settings_changed |=
                                ui.checkbox(&mut c.buddhist, "बौद्ध संवत् Buddhist").changed();
                            settings_changed |=
                                ui.checkbox(&mut c.hebrew, "לוח עברי Hebrew").changed();
                            settings_changed |=
                                ui.checkbox(&mut c.islamic, "هجري Islamic").changed();
                            settings_changed |=
                                ui.checkbox(&mut c.persian, "خورشیدی Persian").changed();
                        }
                    }); // ScrollArea
            });
        if settings_changed {
            self.save_settings();
        }
        // Close via the title-bar ✕, the ⚙ toggle, or Escape.
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if !open || esc {
            self.show_settings.store(false, Ordering::Relaxed);
        }
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
                .with_resizable(false)
                // The main overlay is always-on-top, so the About window must be
                // too — otherwise it opens hidden behind it — and active, so it
                // takes focus and comes to the top of the z-order on open.
                .with_always_on_top()
                .with_active(true),
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
                    .frame(Frame::NONE.fill(pal.bg_deep).inner_margin(Margin::same(20)))
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(6.0);
                            if let Some(tex) = sr {
                                let h = 96.0; // native aspect ~1505×721
                                let img =
                                    egui::Image::new(egui::load::SizedTexture::from_handle(tex))
                                        .fit_to_exact_size(egui::vec2(h * 1505.0 / 721.0, h));
                                if ui
                                    .add(egui::Button::image(img).frame(false))
                                    .on_hover_text("Albert Hui on LinkedIn")
                                    .clicked()
                                {
                                    open_url(AUTHOR_URL);
                                }
                            }
                            ui.add_space(24.0);
                            ui.label(
                                RichText::new("TimeGlyph Lens")
                                    .font(FontId::monospace(16.0))
                                    .color(pal.ink)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("Version {}", timeglyph::VERSION))
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

/// Slim header: the wordmark plus a de-emphasised caption of the source element —
/// context, not the subject (and it keeps sensitive surrounding text from
/// dominating the panel). `caption` is already collapsed and bounded by
/// [`timeglyph_lens::clipboard::caption`]; `None` draws no caption at all.
fn header(
    ui: &mut egui::Ui,
    caption: Option<&str>,
    pal: Palette,
    logo: Option<&egui::TextureHandle>,
    show_settings: &AtomicBool,
    frozen: &AtomicBool,
    no_clipboard: Option<&str>,
) -> bool {
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
        if let Some(caption) = caption {
            ui.add_space(10.0);
            // Single-line Extend over already char-safe text. egui 0.29's
            // Label::truncate() byte-slices the galley and PANICS on multi-byte
            // text (e.g. '·'), so we never use it on arbitrary hovered text.
            ui.add(
                egui::Label::new(
                    RichText::new(caption)
                        .font(FontId::proportional(11.0))
                        .color(pal.faint),
                )
                .wrap_mode(egui::TextWrapMode::Extend),
            );
        }
        // Settings opener, pinned to the top-right corner. Last in the row so the
        // right-to-left layout hugs the corner; reachable on every platform (the
        // ⌘, native menu item is macOS-only).
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("⚙")
                .on_hover_text("settings (click again to close)")
                .clicked()
            {
                // Toggle, so a second click on the gear closes the panel — the
                // reliable close affordance (egui's title-bar ✕ can be invisible
                // against the dark theme).
                let open = show_settings.load(Ordering::Relaxed);
                show_settings.store(!open, Ordering::Relaxed);
            }
            // Freeze toggle, left of the gear. Frozen holds the current reading so
            // the cursor can move onto the overlay to read/copy without the value
            // changing; ▶ resumes cursor-following. Space also toggles it.
            let is_frozen = frozen.load(Ordering::Relaxed);
            let (glyph, tip) = if is_frozen {
                ("▶", "resume following the cursor (Space)")
            } else {
                ("⏸", "freeze the reading (Space)")
            };
            if ui.button(glyph).on_hover_text(tip).clicked() {
                frozen.store(!is_frozen, Ordering::Relaxed);
            }
            // Clipboard decode, left of the freeze toggle. One read per press: the
            // press is the consent, so nothing watches the clipboard between them.
            //
            // 🗐 U+1F5D0, not 📋 U+1F4CB: both are in egui's bundled fonts, but
            // NotoEmoji wins the fallback order for U+1F4CB and its clipboard is
            // drawn tilted — at 14 px it reads as a luggage tag. U+1F5D0 falls
            // through to emoji-icon-font (the same face that draws ⏸ and ⚙) and
            // renders the familiar overlapping-pages copy mark, identically on
            // every platform because the font ships with egui.
            let btn = egui::Button::new("🗐");
            match no_clipboard {
                None => ui
                    .add(btn)
                    .on_hover_text(
                        "decode the clipboard — for a value hovering can't reach, \
                         such as one inside a VM guest window",
                    )
                    .clicked(),
                // No clipboard on this host: say so, rather than offering a button
                // whose every press would look like an empty clipboard.
                Some(reason) => {
                    ui.add_enabled(false, btn).on_disabled_hover_text(reason);
                    false
                }
            }
        })
        .inner
    })
    .inner
}

/// Grid column 1: the amber format chip. The verbose format name is a hover
/// tooltip on the chip (not an always-shown line), keeping each reading compact.
fn chip_cell(ui: &mut egui::Ui, r: &Reading, pal: Palette) {
    ui.horizontal(|ui| {
        // Uniform row height (see `row_h`) so the format chip centers on the same
        // midline as every other cell and the dots stay evenly spaced.
        ui.set_min_height(row_h(ui));
        Frame::NONE
            .fill(pal.bg_chip)
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::symmetric(6, 2))
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
    ui.fonts_mut(|f| {
        f.row_height(&FontId::proportional(11.0)) + f.row_height(&FontId::proportional(10.0))
    })
}

fn datetime_cell(
    ui: &mut egui::Ui,
    r: &Reading,
    zone: &RenderZone,
    style: DateStyle,
    pal: Palette,
) {
    // The displayed datetime, in the chosen style. Local-naive readings keep
    // their own wall-clock rendering (never shifted); the styled form is only for
    // the zone-shiftable case. `copy_text_for` owns this choice so the
    // click-to-copy string always matches what is shown. The weekday / holiday
    // labels derive from `text::label_basis`, an ISO rendering of the same instant
    // in the SAME zone — deriving them from the reading's baked `r.rendered`
    // labelled the date with whichever zone was active at decode time.
    let shown = text::copy_text_for(r, zone, style);
    // The ISO rendering of the same instant in the SAME zone, for every label derived
    // from the date: weekday, public holiday, and the UTC designator. Deriving those
    // from the reading's baked `r.rendered` described whichever zone was active when
    // the reading was decoded.
    let basis = text::label_basis(r, zone);
    let datetime = || {
        RichText::new(&shown)
            .font(FontId::monospace(14.0))
            .color(pal.ink)
    };
    // The whole row is click-to-copy: clicking copies the shown datetime to the
    // clipboard via egui's platform output (ctx.copy_text → PlatformOutput
    // .copied_text, egui 0.29). A pointing-hand cursor + tooltip advertise it.
    let resp = ui.horizontal(|ui| {
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
            if let Some(wd) = basis.as_deref().and_then(scan::weekday) {
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
        } else if basis.as_deref().is_some_and(|b| b.ends_with('Z')) {
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
        if let Some(wd) = basis.as_deref().and_then(scan::weekday) {
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
        if let Some(name) = basis
            .as_deref()
            .and_then(|b| timeglyph::holiday::in_zone_rendered(zone, b))
        {
            ui.add_space(6.0);
            ui.label(
                RichText::new(name)
                    .font(FontId::proportional(11.0))
                    .color(pal.amber)
                    .strong(),
            );
        }
    });
    let resp = resp
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("click to copy");
    if resp.clicked() {
        ui.ctx().copy_text(shown);
    }
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

/// The Hebrew + Islamic calendar dates for a reading, on one faint line beneath
/// the 干支 row — the lens counterpart to the `cal` day card's alt-calendar rows.
fn altcal_cell(
    ui: &mut egui::Ui,
    instant: PosixNs,
    zone: &RenderZone,
    cals: persist::CalendarVisibility,
    pal: Palette,
) {
    let Some(v) = altcal::altcal_view(instant, zone) else {
        ui.label("");
        return;
    };
    // Only the calendars enabled in settings (matched by stable key). Each row's
    // bilingual label is long, so show one per line for readability.
    let line = |ui: &mut egui::Ui, text: &str| {
        ui.label(
            RichText::new(text)
                .font(FontId::proportional(10.5))
                .color(pal.faint),
        );
    };
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for row in v.calendars.iter().filter(|r| cals.shows(&r.key)) {
            line(ui, &row.label);
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
                p.rect_filled(rect, CornerRadius::same(4), pal.bg_deep);
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

// Offscreen (headless wgpu) egui_kittest render gate for `LensApp`. In-crate so
// it can reach the private app + font/theme helpers; see the module's own docs.
//
// Compiled on EVERY platform on purpose: the all-black / tofu / uniform-frame
// assertion is genuinely platform-independent, and the regression it catches is the
// load-bearing one (`default_fonts` dropped ⇒ a silent all-black window — see
// Cargo.toml). Only the *pixel compare* against the macOS-rendered reference PNG is
// macOS-gated, inside the module.
#[cfg(test)]
#[path = "overlay_snapshot_test.rs"]
mod overlay_snapshot_test;
