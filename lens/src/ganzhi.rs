//! Format timeglyph's lunisolar / 干支 reading for the overlay's opt-in
//! expansion. Pure over the (feature-gated) engine; the egui disclosure is the
//! shell. Requires the `timeglyph` `lunisolar` feature (enabled in Cargo.toml).

use timeglyph::{calfmt, lunisolar, PosixNs, RenderZone};

// The 五行 (Five Element) assignment lives in the shared library formatter so the
// lens and the `cal` subcommand never drift; re-exported here for the overlay.
pub use timeglyph::calfmt::{five_element, Element};

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
    /// The sun's apparent ecliptic longitude (°). The solar term is the 15° arc
    /// this falls in, so λ shows the position *within* the term (a period), not
    /// that today is the term's exact day.
    pub solar_longitude_deg: f64,
    /// Calendar days since the solar term began (`0` = the term's own day).
    pub days_into_term: u32,
    /// Stated assumptions (meridian, pillar conventions, solar-time note) — a
    /// reading, not a verdict.
    pub assumptions: Vec<String>,
}

impl GanzhiView {
    /// The solar term as a period phrase: the bare term on its own day
    /// (`冬至`), else `<term>後第<N>日` (`冬至後第十日`) — so a day well past the
    /// term is not misread as the term's day.
    #[must_use]
    pub fn solar_term_phrase(&self) -> String {
        calfmt::solar_term_phrase(&self.solar_term, self.days_into_term)
    }
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
            calfmt::lunar_month_cn(r.lunar_month, r.is_leap_month),
            calfmt::lunar_day_cn(r.lunar_day)
        ),
        solar_term: r.solar_term,
        solar_longitude_deg: r.solar_longitude_deg,
        days_into_term: r.days_into_term,
        assumptions: r.assumptions,
    })
}

/// Parse a longitude entry (°E, east positive) for the hour-pillar correction.
/// Empty, non-numeric, or out-of-range (beyond ±180) → `None` (no correction).
#[must_use]
pub fn parse_longitude(s: &str) -> Option<f64> {
    let v: f64 = s.trim().parse().ok()?;
    (v.is_finite() && v.abs() <= 180.0).then_some(v)
}
