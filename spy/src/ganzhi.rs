//! Format timeglyph's lunisolar / 干支 reading for the overlay's opt-in
//! expansion. Pure over the (feature-gated) engine; the egui disclosure is the
//! shell. Requires the `timeglyph` `lunisolar` feature (enabled in Cargo.toml).

use timeglyph::{lunisolar, PosixNs, RenderZone};

/// A 干支 / lunisolar view of one instant, ready to render as rows.
#[derive(Debug, Clone)]
pub struct GanzhiView {
    /// Year pillar (年柱), e.g. `庚子`.
    pub year_pillar: String,
    /// Month pillar (月柱).
    pub month_pillar: String,
    /// Day pillar (日柱).
    pub day_pillar: String,
    /// Hour pillar (時柱) — the only pillar the longitude correction can move.
    pub hour_pillar: String,
    /// The civil lunisolar date, e.g. `1999 年 11 月 06 日` (leap months marked 闰).
    pub lunar_date: String,
    /// The current solar term (節氣).
    pub solar_term: String,
    /// Stated assumptions (meridian, pillar conventions, solar-time note) — a
    /// reading, not a verdict.
    pub assumptions: Vec<String>,
}

/// Compute the 干支 view for `instant` at the meridian `zone`, optionally
/// correcting the HOUR pillar to local mean solar time at `longitude` (°E, east
/// positive). The Chinese date is meridian-relative, so the display zone the
/// analyst has chosen serves as that meridian. Returns the engine error as a
/// string if the instant cannot be rendered.
pub fn ganzhi_view(
    instant: PosixNs,
    zone: &RenderZone,
    longitude: Option<f64>,
) -> Result<GanzhiView, String> {
    let r = lunisolar::render(instant, zone, longitude).map_err(|e| e.to_string())?;
    Ok(GanzhiView {
        year_pillar: r.year_pillar,
        month_pillar: r.month_pillar,
        day_pillar: r.day_pillar,
        hour_pillar: r.hour_pillar,
        // Chinese lunar notation (十二月初七), NOT "2019 年 12 月 07 日" — the arabic
        // form reads as a Gregorian date and misleads (it is a *lunar* date).
        lunar_date: format!(
            "{}年 {}{}",
            r.lunar_year,
            lunar_month_name(r.lunar_month, r.is_leap_month),
            lunar_day_name(r.lunar_day)
        ),
        solar_term: r.solar_term,
        assumptions: r.assumptions,
    })
}

/// The Chinese lunar month name (`正月`, `二月`…`十二月`; leap months prefixed
/// `閏`), unambiguously a lunar month — not a Gregorian numeral.
fn lunar_month_name(month: u8, leap: bool) -> String {
    const NAMES: [&str; 12] = [
        "正", "二", "三", "四", "五", "六", "七", "八", "九", "十", "十一", "十二",
    ];
    let base = NAMES
        .get((month as usize).wrapping_sub(1))
        .copied()
        .unwrap_or("?");
    format!("{}{base}月", if leap { "閏" } else { "" })
}

/// The Chinese lunar day name (`初一`…`初十`, `十一`…`二十`, `廿一`…`三十`).
fn lunar_day_name(day: u8) -> String {
    const D: [&str; 10] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    match day {
        1..=9 => format!("初{}", D[day as usize]),
        10 => "初十".to_string(),
        11..=19 => format!("十{}", D[(day - 10) as usize]),
        20 => "二十".to_string(),
        21..=29 => format!("廿{}", D[(day - 20) as usize]),
        30 => "三十".to_string(),
        other => other.to_string(),
    }
}

/// Parse a longitude entry (°E, east positive) for the hour-pillar correction.
/// Empty, non-numeric, or out-of-range (beyond ±180) → `None` (no correction).
#[must_use]
pub fn parse_longitude(s: &str) -> Option<f64> {
    let v: f64 = s.trim().parse().ok()?;
    (v.is_finite() && v.abs() <= 180.0).then_some(v)
}
