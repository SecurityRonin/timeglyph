//! The live overlay (eframe/egui): an always-on-top window that follows the
//! cursor and shows timeglyph's readings for any number in the element under it.
//! Cross-platform — the same window on Windows and macOS; only the [`Picker`] is
//! platform-specific.
//!
//! Presentation is a compact "time-instrument" inspector panel: each number is
//! the subject; each candidate reading a `format · instant · label` row with an
//! opt-in 干支 expansion; the raw source element a de-emphasised caption; and a
//! footer selects the display timezone (UTC by default, any other zone "loud").
//! High-contrast warm-dark palette (WCAG AA on `BG_DEEP`) with a brass accent.

use std::time::Duration;

use eframe::egui;
use egui::{Color32, FontId, Frame, Margin, RichText, Rounding, Stroke};
use timeglyph::{PosixNs, RenderZone};
use timeglyph_spy::zone::{self, parse_zone, ZoneChoice};
use timeglyph_spy::{ganzhi, text, tzinfo, tzmap};

use crate::picker::Picker;
use crate::scan::{self, NumberReadings, Reading};

// Warm "time instrument" palette. Text contrasts vs BG_DEEP (#14120F):
// INK ~15:1, AMBER ~10:1, MUTE ~8:1, FAINT ~5:1 — all clear WCAG AA.
const BG_DEEP: Color32 = Color32::from_rgb(20, 18, 15); // warm near-black
const BG_CARD: Color32 = Color32::from_rgb(31, 28, 22);
const BG_CHIP: Color32 = Color32::from_rgb(38, 31, 20); // amber-tinted
const HAIRLINE: Color32 = Color32::from_rgb(52, 47, 38);
const INK: Color32 = Color32::from_rgb(245, 241, 232); // warm white — datetime values
const AMBER: Color32 = Color32::from_rgb(240, 180, 41); // brass accent — format + pillars
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
            install_theme(&cc.egui_ctx);
            Ok(Box::new(SpyApp::new(picker)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// Append the OS fallback fonts (a CJK face for the 干支 pillars + lunar date, a
/// symbol face for the chrome's ◷ / ⚠) to both families. egui's bundled fonts
/// carry no CJK/symbol glyphs, so without this those render as missing-glyph
/// boxes (tofu); which glyphs the stack must cover is asserted by
/// `tests/fonts.rs`. Loaded at runtime (not bundled); if the host has neither,
/// the overlay still runs and only the uncovered glyphs degrade to tofu.
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
    /// The display timezone. Session-scoped, UTC by default: it never persists
    /// across launches, so a prior case's zone can't silently apply to the next.
    zone: ZoneChoice,
    /// Cached IANA continents (first level of the Continent → Zone picker).
    continents: Vec<String>,
    /// Selected continent and its zones (second level of the picker).
    continent: String,
    zones: Vec<String>,
    /// Optional global longitude (°E): refines every reading's 干支 hour pillar to
    /// true solar time. Its footer entry buffer.
    longitude: Option<f64>,
    longitude_input: String,
    /// Whether the clickable world map window is open, and the picked UTC offset
    /// (used to highlight the whole band, not a single polygon).
    show_map: bool,
    map_pick: Option<f64>,
}

impl SpyApp {
    fn new(picker: Picker) -> Self {
        Self {
            picker,
            last_text: String::new(),
            source: String::new(),
            hits: Vec::new(),
            zone: ZoneChoice::default(),
            continents: zone::continents(),
            continent: String::new(),
            zones: Vec::new(),
            longitude: None,
            longitude_input: String::new(),
            show_map: false,
            map_pick: None,
        }
    }
}

impl eframe::App for SpyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut dirty = false;
        let text = self.picker.text_under_cursor().unwrap_or_default();
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
                    .fill(BG_DEEP)
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

        // Re-decode when either the hovered text OR the display zone changed.
        if dirty {
            self.hits = scan::inspect_text(&self.source, MAX_READINGS, &self.zone.zone);
        }

        // Snapshot into locals so the nested render closures capture no `self`.
        let source = self.source.clone();
        let hits = std::mem::take(&mut self.hits);
        let zone = self.zone.zone.clone();
        let longitude = self.longitude;

        let panel = Frame::none()
            .fill(BG_DEEP)
            .inner_margin(Margin::symmetric(16.0, 14.0));
        egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
            header(ui, &source);
            ui.separator();
            ui.add_space(10.0);
            if hits.is_empty() {
                empty_state(
                    ui,
                    "Hover an element with a number",
                    "Point at any on-screen value to decode it",
                );
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for nr in &hits {
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
                                    // 2-column grid: the format chip is column 1
                                    // (a tab stop), so every datetime — and the
                                    // 干支 line beneath it — left-aligns in column 2.
                                    egui::Grid::new(nr.number.as_str())
                                        .num_columns(2)
                                        .spacing([10.0, 8.0])
                                        .show(ui, |ui| {
                                            for r in &nr.readings {
                                                chip_cell(ui, r);
                                                datetime_cell(ui, r, &zone);
                                                ui.end_row();
                                                ui.label(""); // empty col 1
                                                ganzhi_cell(ui, r.instant, &zone, longitude);
                                                ui.end_row();
                                            }
                                        });
                                });
                            ui.add_space(10.0);
                        }
                    });
            }
        });

        self.hits = hits;

        // Poll the cursor a few times a second without busy-spinning.
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

/// The active-zone summary shown in the footer chip: `⚠ Europe/London ·
/// UTC+01:00 BST · DST` for a named zone at `at`, or `UTC`. Because offset/DST
/// are per-instant, the summary is resolved at the reference instant `at`.
fn zone_summary(zone: &ZoneChoice, at: PosixNs) -> String {
    match tzinfo::stamp(&zone.zone, at) {
        Some(s) => {
            let abbr = if s.abbr.is_empty() {
                String::new()
            } else {
                format!(" {}", s.abbr)
            };
            let dst = if s.dst { " · DST" } else { "" };
            format!("⚠ {} · UTC{}{abbr}{dst}", zone.label, s.offset)
        }
        None => zone.label.clone(),
    }
}

/// Current wall-clock instant, used as the footer's reference when no reading is
/// on screen yet.
fn now_instant() -> PosixNs {
    PosixNs(jiff::Timestamp::now().as_nanosecond())
}

impl SpyApp {
    /// The footer time-zone control: an active summary (offset · abbr · DST at the
    /// reference instant `at`), UTC/Local presets, and a Continent → Zone picker
    /// (Windows-style, offset shown at selection). Returns `true` when the zone
    /// changed. UTC is calm; any other zone renders "loud" (amber ⚠).
    fn zone_footer(&mut self, ui: &mut egui::Ui, at: PosixNs) -> bool {
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("time zone")
                    .font(FontId::proportional(11.0))
                    .color(FAINT),
            );
            let (fill, fg) = if self.zone.loud {
                (BG_CHIP, AMBER)
            } else {
                (BG_CARD, MUTE)
            };
            let summary = zone_summary(&self.zone, at);
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
                self.continent.clear();
                self.map_pick = None;
                changed = true;
            }
            if ui.small_button("Local").clicked() {
                if let Some(z) = parse_zone("local") {
                    self.zone = z;
                    changed = true;
                }
            }
            if ui.small_button("map").clicked() {
                self.show_map = !self.show_map;
            }
            // Continent → Zone picker. Iterate over clones so the closures don't
            // alias the fields they mutate.
            let conts = self.continents.clone();
            egui::ComboBox::from_id_salt("tz_continent")
                .selected_text(if self.continent.is_empty() {
                    "Region…".to_string()
                } else {
                    self.continent.clone()
                })
                .show_ui(ui, |ui| {
                    for c in &conts {
                        if ui.selectable_label(&self.continent == c, c).clicked() {
                            self.continent = c.clone();
                            self.zones = zone::zones_in(c);
                        }
                    }
                });
            let zones = self.zones.clone();
            if !zones.is_empty() {
                egui::ComboBox::from_id_salt("tz_zone")
                    .selected_text("Zone…")
                    .show_ui(ui, |ui| {
                        for z in &zones {
                            if ui
                                .selectable_label(false, zone::menu_label(z, at))
                                .clicked()
                            {
                                if let Some(zc) = parse_zone(z) {
                                    self.zone = zc;
                                    changed = true;
                                }
                            }
                        }
                    });
            }

            // Global longitude (°E): refines every reading's 干支 hour pillar to
            // true solar time. Optional — empty/invalid means no correction.
            ui.add_space(8.0);
            ui.label(
                RichText::new("long")
                    .font(FontId::proportional(11.0))
                    .color(FAINT),
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
        });
        changed
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
            // Char-safe truncation + single-line Extend. egui 0.29's
            // Label::truncate() byte-slices the galley and PANICS on multi-byte
            // text (e.g. '·'), so we never use it on arbitrary hovered text.
            let collapsed: String = source.split_whitespace().collect::<Vec<_>>().join(" ");
            ui.add(
                egui::Label::new(
                    RichText::new(text::ellipsize(&collapsed, 120))
                        .font(FontId::proportional(11.0))
                        .color(FAINT),
                )
                .wrap_mode(egui::TextWrapMode::Extend),
            );
        }
    });
}

/// Grid column 1: the amber format chip. The verbose format name is a hover
/// tooltip on the chip (not an always-shown line), keeping each reading compact.
fn chip_cell(ui: &mut egui::Ui, r: &Reading) {
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
        })
        .response
        .on_hover_text(r.label.as_str());
}

/// Grid column 2 (row 1): the rendered instant, the per-instant abbreviation and
/// DST (the numeric offset is already in `rendered`; a location alone is
/// ambiguous, so these disambiguate), and an optional local tag.
fn datetime_cell(ui: &mut egui::Ui, r: &Reading, zone: &RenderZone) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&r.rendered)
                .font(FontId::monospace(14.0))
                .color(INK),
        );
        if !r.local {
            if let Some(s) = tzinfo::stamp(zone, r.instant) {
                if !s.abbr.is_empty() {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&s.abbr)
                            .font(FontId::monospace(11.0))
                            .color(MUTE),
                    );
                }
                if s.dst {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("DST")
                            .font(FontId::proportional(10.0))
                            .color(AMBER)
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
                    .color(FAINT),
            );
        }
    });
}

/// The 干支 / lunisolar reading for one instant, shown compactly beneath every
/// reading — ALWAYS visible, resolved at the display zone (the meridian) and
/// refined by the optional global longitude (hour pillar → true solar time).
/// A reading, not a verdict. Silently omitted if the engine cannot render it.
/// Grid column 2 (row 2): the 干支 line, led by the lunar date so it left-aligns
/// under the datetime above it, then the four pillars. Empty cell if the engine
/// can't render (keeps the grid row intact).
fn ganzhi_cell(ui: &mut egui::Ui, instant: PosixNs, zone: &RenderZone, longitude: Option<f64>) {
    let Ok(v) = ganzhi::ganzhi_view(instant, zone, longitude) else {
        ui.label("");
        return;
    };
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{} · {}", v.lunar_date, v.solar_term_phrase()))
                .font(FontId::proportional(10.5))
                .color(FAINT),
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
                    .color(MUTE),
            );
            ui.add_space(5.0);
        }
    });
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

/// Format a UTC offset (hours) as a `parse_zone` spec: `-5.0` → `-05:00`,
/// `5.5` → `+05:30`, `0.0` → `UTC`.
fn offset_spec(off: f64) -> String {
    if off == 0.0 {
        return "UTC".to_string();
    }
    let sign = if off < 0.0 { '-' } else { '+' };
    let a = off.abs();
    let h = a.trunc() as u32;
    let m = ((a - a.trunc()) * 60.0).round() as u32;
    format!("{sign}{h:02}:{m:02}")
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
                    .color(FAINT),
                );
                let w = ui.available_width();
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(w, w / 2.0), egui::Sense::click());
                let p = ui.painter_at(rect);
                p.rect_filled(rect, Rounding::same(4.0), BG_DEEP);
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
                        Stroke::new(1.4, AMBER)
                    } else {
                        Stroke::new(0.4, HAIRLINE)
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
                                .unwrap_or_else(|| offset_spec(pick.offset));
                            if let Some(z) = parse_zone(&spec) {
                                self.zone = z;
                                self.map_pick = Some(pick.offset);
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
        return Color32::from_rgb(120, 84, 24);
    }
    let k = u8::try_from((offset.round() as i64).rem_euclid(3)).unwrap_or(0);
    let b = 30 + k * 8;
    Color32::from_rgb(b, b.saturating_sub(3), b.saturating_sub(10))
}
