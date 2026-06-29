//! Chinese lunisolar calendar + 干支 (Heavenly-Stem / Earthly-Branch) four-pillar
//! rendering, behind the `lunisolar` feature.
//!
//! Unlike the rest of timeglyph (a pure instant↔instant mapping), this
//! conversion is **convention-relative**: a UTC instant maps to a lunisolar date
//! only once a *reference meridian* is fixed (China uses UTC+8; Vietnam UTC+7;
//! Korea UTC+9), because the calendar assigns astronomical new-moon / solar-term
//! instants to civil **days** at that meridian. So [`render`] REQUIRES a
//! [`RenderZone`]. The optional `longitude` applies a local-mean-solar-time
//! correction to the HOUR pillar only (真太陽時) — the one field traditionally
//! reckoned by the observer's solar clock; the equation of time is NOT applied
//! (stated in the reading's assumptions).
//!
//! The ephemeris (new-moon / solar-term astronomy) and calendar rules are
//! delegated to `lunar-lite` (reuse, don't reinvent); timeglyph supplies the
//! meridian/longitude that crate intentionally leaves to the caller. Validated
//! against the independent `cnlunar` oracle (tests/lunisolar.rs).

use lunar_lite::{
    four_pillars_from_solar_date, solar_to_lunar, time_index, FourPillars, SolarDate, StemBranch,
};

use crate::{ChronoError, PosixNs, RenderZone};

/// The ten Heavenly Stems (天干), indexed 0..=9.
const STEMS: [char; 10] = ['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸'];
/// The twelve Earthly Branches (地支), indexed 0..=11.
const BRANCHES: [char; 12] = [
    '子', '丑', '寅', '卯', '辰', '巳', '午', '未', '申', '酉', '戌', '亥',
];

/// A lunisolar / 干支 reading of an instant at a chosen meridian. The lunar date
/// is the civil Chinese-calendar date; the four pillars are the sexagenary
/// year/month/day/hour columns. Carries its assumptions — a reading, not a
/// verdict (the meridian and pillar conventions are choices, surfaced here).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LunisolarReading {
    /// Chinese lunar year.
    pub lunar_year: i32,
    /// Lunar month, 1..=12.
    pub lunar_month: u8,
    /// Day of the lunar month, 1..=30.
    pub lunar_day: u8,
    /// Whether this is the leap (intercalary) instance of the month.
    pub is_leap_month: bool,
    /// Year pillar (年柱), e.g. `庚子`.
    pub year_pillar: String,
    /// Month pillar (月柱).
    pub month_pillar: String,
    /// Day pillar (日柱).
    pub day_pillar: String,
    /// Hour pillar (時柱).
    pub hour_pillar: String,
    /// The civil datetime at the chosen meridian (RFC 3339 with offset).
    pub civil_local: String,
    /// Stated assumptions (meridian used, pillar conventions, solar-time note).
    pub assumptions: Vec<String>,
}

/// Render an `instant` as a Chinese lunisolar / 干支 reading at the `zone`
/// meridian, optionally correcting the hour pillar to true (mean) solar time at
/// `longitude` degrees east.
///
/// `zone` is **required** (the conversion is meridian-relative). `longitude`,
/// when given, shifts only the hour pillar; the lunar date and year/month/day
/// pillars stay on the civil meridian day.
pub fn render(
    instant: PosixNs,
    zone: &RenderZone,
    longitude: Option<f64>,
) -> Result<LunisolarReading, ChronoError> {
    let ts = jiff::Timestamp::from_nanosecond(instant.0)
        .map_err(|e| ChronoError::Render(e.to_string()))?;
    // The meridian's UTC offset at this instant (DST-resolved for named zones).
    let offset = match zone {
        RenderZone::Utc => jiff::tz::Offset::UTC,
        RenderZone::Fixed(o) => *o,
        RenderZone::Named(tz) => tz.to_offset(ts),
    };
    let dt = offset.to_datetime(ts);
    let solar = SolarDate {
        year: i32::from(dt.year()),
        month: dt.month() as u8,
        day: dt.day() as u8,
    };
    let hour = i64::from(dt.hour());
    let minute = i64::from(dt.minute());

    // Lunar date + the civil-time four pillars (year via 立春, month via 节).
    let lunar = solar_to_lunar(solar).map_err(|e| ChronoError::Render(e.to_string()))?;
    let ti_civil =
        time_index(hour as u8, minute as u8).map_err(|e| ChronoError::Render(e.to_string()))?;
    let base: FourPillars = four_pillars_from_solar_date(solar, ti_civil)
        .map_err(|e| ChronoError::Render(e.to_string()))?;

    // Optional true-solar-time correction, applied to the HOUR pillar only.
    let ref_lon = f64::from(offset.seconds()) / 3600.0 * 15.0;
    let (hour_pillar, solar_note) = match longitude {
        Some(lon) => {
            let corr_min = ((lon - ref_lon) * 4.0).round() as i64;
            let wrapped = (hour * 60 + minute + corr_min).rem_euclid(24 * 60);
            let ti_solar = time_index((wrapped / 60) as u8, (wrapped % 60) as u8)
                .map_err(|e| ChronoError::Render(e.to_string()))?;
            let hp = four_pillars_from_solar_date(solar, ti_solar)
                .map_err(|e| ChronoError::Render(e.to_string()))?
                .hourly;
            let note = format!(
                "hour pillar uses local MEAN solar time (longitude {lon:.4}°E vs meridian {ref_lon:.1}°E, {corr_min:+} min); the equation of time is NOT applied, and the day pillar stays on the civil meridian day"
            );
            (pillar_string(hp), note)
        }
        None => (
            pillar_string(base.hourly),
            "hour pillar uses civil time at the meridian (no longitude → true solar time not applied)".to_string(),
        ),
    };

    let assumptions = vec![
        format!(
            "Chinese lunisolar reading computed for the {ref_lon:.1}°E meridian (UTC offset {} h); a different tradition (e.g. Vietnam UTC+7, Korea UTC+9) can differ by a day or a leap month",
            offset.seconds() / 3600
        ),
        "year pillar uses the 立春 (LiChun) boundary and month pillar the 12 节 solar terms (orthodox 子平 convention); the lunar DATE uses the 正月初一 new-year boundary, so the year pillar and lunar year may differ near 立春".to_string(),
        solar_note,
    ];

    Ok(LunisolarReading {
        lunar_year: lunar.year,
        lunar_month: lunar.month,
        lunar_day: lunar.day,
        is_leap_month: lunar.is_leap_month,
        year_pillar: pillar_string(base.yearly),
        month_pillar: pillar_string(base.monthly),
        day_pillar: pillar_string(base.daily),
        hour_pillar,
        civil_local: instant
            .render(zone)
            .unwrap_or_else(|| "<out of civil range>".to_string()),
        assumptions,
    })
}

/// Render a [`StemBranch`] as its two-character 干支 string (e.g. `庚子`).
fn pillar_string(sb: StemBranch) -> String {
    let mut s = String::with_capacity(6);
    s.push(STEMS[sb.stem().index() % 10]);
    s.push(BRANCHES[sb.branch().index() % 12]);
    s
}
