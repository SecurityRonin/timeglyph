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
    /// The display timezone. Session-scoped, UTC by default: it never persists
    /// across launches, so a prior case's zone can't silently apply to the next.
    zone: ZoneChoice,
    /// Cached IANA continents (first level of the Continent → Zone picker).
    continents: Vec<String>,
    /// Selected continent and its zones (second level of the picker).
    continent: String,
    zones: Vec<String>,
    /// Which (number, reading) has its 干支 expansion open, if any.
    expanded: Option<(usize, usize)>,
    /// Optional longitude (°E) for the hour-pillar correction, and its buffer.
    longitude: Option<f64>,
    longitude_input: String,
    /// Whether the clickable world map window is open, and the last-picked region.
    show_map: bool,
    map_pick: Option<usize>,
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
            expanded: None,
            longitude: None,
            longitude_input: String::new(),
            show_map: false,
            map_pick: None,
        }
    }
}

impl eframe::App for SpyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let text = self.picker.text_under_cursor().unwrap_or_default();
        let mut dirty = false;
        if text != self.last_text {
            self.last_text.clone_from(&text);
            self.source = text.clone();
            dirty = true;
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
        let expanded = self.expanded;
        let mut new_expanded = expanded;
        let mut longitude = self.longitude;
        let mut lon_input = std::mem::take(&mut self.longitude_input);

        let panel = Frame::none()
            .fill(BG_DEEP)
            .inner_margin(Margin::symmetric(16.0, 14.0));
        egui::CentralPanel::default().frame(panel).show(ctx, |ui| {
            header(ui, &source);
            ui.separator();
            ui.add_space(10.0);
            if source.is_empty() {
                empty_state(
                    ui,
                    "Hover an element with a number",
                    "Point at any on-screen value to decode it",
                );
            } else if hits.is_empty() {
                empty_state(
                    ui,
                    "No timestamp-like number here",
                    "This element has no value that reads as a date",
                );
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (ni, nr) in hits.iter().enumerate() {
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
                                    for (ri, r) in nr.readings.iter().enumerate() {
                                        if ri > 0 {
                                            ui.add_space(8.0);
                                        }
                                        let key = (ni, ri);
                                        let open = expanded == Some(key);
                                        if reading_row(ui, r, open, &zone) {
                                            new_expanded = if open { None } else { Some(key) };
                                        }
                                        if open {
                                            ganzhi_expansion(ui, r.instant, &zone, longitude);
                                            longitude_row(ui, &mut lon_input, &mut longitude);
                                        }
                                    }
                                });
                            ui.add_space(10.0);
                        }
                    });
            }
        });

        self.hits = hits;
        self.longitude_input = lon_input;
        self.longitude = longitude;
        self.expanded = new_expanded;

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
            if ui.small_button("🗺 map").clicked() {
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

/// One reading: an amber format chip, the rendered instant, an optional local
/// tag, and a 干支 disclosure toggle. Returns `true` when the toggle was clicked.
/// Role is conveyed by position and weight, not colour alone.
fn reading_row(ui: &mut egui::Ui, r: &Reading, open: bool, zone: &RenderZone) -> bool {
    let mut toggled = false;
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
        // Per-instant abbreviation + DST (the numeric offset is already in
        // `rendered`). A location alone is ambiguous, so these disambiguate.
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
        let arrow = if open { "▾" } else { "▸" };
        if ui
            .small_button(
                RichText::new(format!("{arrow}干支"))
                    .font(FontId::proportional(11.0))
                    .color(MUTE),
            )
            .clicked()
        {
            toggled = true;
        }
    });
    ui.label(
        RichText::new(&r.label)
            .font(FontId::proportional(11.5))
            .color(MUTE),
    );
    toggled
}

/// The 干支 / lunisolar expansion for one reading's instant, using the current
/// display zone as the meridian. A reading, not a verdict — the assumptions
/// (meridian, conventions) are surfaced beneath the pillars.
fn ganzhi_expansion(
    ui: &mut egui::Ui,
    instant: PosixNs,
    zone: &RenderZone,
    longitude: Option<f64>,
) {
    ui.add_space(6.0);
    Frame::none()
        .fill(BG_DEEP)
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .stroke(Stroke::new(1.0, HAIRLINE))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            match ganzhi::ganzhi_view(instant, zone, longitude) {
                Ok(v) => {
                    ui.horizontal_wrapped(|ui| {
                        for (mark, pillar) in [
                            ("年", &v.year_pillar),
                            ("月", &v.month_pillar),
                            ("日", &v.day_pillar),
                            ("時", &v.hour_pillar),
                        ] {
                            ui.label(
                                RichText::new(format!("{mark} {pillar}"))
                                    .font(FontId::monospace(15.0))
                                    .color(AMBER)
                                    .strong(),
                            );
                            ui.add_space(10.0);
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("{}  ·  {}", v.lunar_date, v.solar_term))
                            .font(FontId::monospace(12.0))
                            .color(INK),
                    );
                    for a in &v.assumptions {
                        ui.label(
                            RichText::new(format!("— {a}"))
                                .font(FontId::proportional(10.5))
                                .color(FAINT),
                        );
                    }
                }
                Err(e) => {
                    ui.label(
                        RichText::new(format!("干支 unavailable: {e}"))
                            .font(FontId::proportional(11.0))
                            .color(FAINT),
                    );
                }
            }
        });
}

/// The optional longitude entry inside a 干支 expansion. Live-parses into
/// `longitude` (empty / invalid / out-of-range → no correction).
fn longitude_row(ui: &mut egui::Ui, input: &mut String, longitude: &mut Option<f64>) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("longitude °E")
                .font(FontId::proportional(11.0))
                .color(FAINT),
        );
        let resp = ui.add(
            egui::TextEdit::singleline(input)
                .hint_text("e.g. 121.5 (optional)")
                .desired_width(120.0)
                .font(FontId::monospace(12.0)),
        );
        if resp.changed() {
            *longitude = ganzhi::parse_longitude(input);
        }
        if let Some(l) = longitude {
            ui.label(
                RichText::new(format!("→ 真太陽時 @ {l}°E"))
                    .font(FontId::proportional(10.5))
                    .color(MUTE),
            );
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
                for (i, r) in tzmap::regions().iter().enumerate() {
                    let stroke = if self.map_pick == Some(i) {
                        Stroke::new(1.6, AMBER)
                    } else {
                        Stroke::new(0.4, HAIRLINE)
                    };
                    for ring in &r.rings {
                        let pts: Vec<egui::Pos2> = ring.iter().map(|c| proj(c[0], c[1])).collect();
                        p.add(egui::Shape::closed_line(pts, stroke));
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
                                self.map_pick = tzmap::regions().iter().position(|rr| {
                                    rr.offset == pick.offset && rr.iana == pick.iana
                                });
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
