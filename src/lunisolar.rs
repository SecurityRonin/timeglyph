//! Chinese lunisolar calendar + 干支 (Heavenly-Stem / Earthly-Branch) four-pillar
//! rendering, behind the `lunisolar` feature.
//!
//! Unlike the rest of timeglyph (a pure instant↔instant mapping), this
//! conversion is **convention-relative**: a UTC instant maps to a lunisolar date
//! only once a *reference meridian* is fixed (China uses UTC+8; Vietnam UTC+7;
//! Korea UTC+9), because the calendar assigns civil DAYS at that meridian. So
//! [`render`] REQUIRES a [`RenderZone`]. The optional `longitude` corrects the
//! HOUR pillar to local mean solar time (真太陽時); the equation of time is NOT
//! applied (stated in the reading's assumptions).
//!
//! All astronomy is delegated to the **`stem-branch`** crate (reuse, don't
//! reinvent): the VSOP87D solar + ELP/MPP02 lunar ephemeris, new-moon /
//! solar-term / 中氣 leap-month rules, ΔT, and Julian-day math. timeglyph adds
//! only the convention layer — the meridian, the optional longitude correction,
//! and the day/hour pillar arithmetic (Julian-day count + 五鼠遁). The whole
//! reading is validated against the independent `cnlunar` oracle
//! (tests/lunisolar.rs).

use std::panic::AssertUnwindSafe;

use stem_branch::{
    delta_t_for_year, gregorian_to_lunisolar, jd_from_ymd, solar_ecliptic_state, CivilDate,
};

use crate::{ChronoError, PosixNs, RenderZone};

/// The ten Heavenly Stems (天干), indexed 0..=9.
const STEMS: [char; 10] = ['甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸'];
/// The twelve Earthly Branches (地支), indexed 0..=11.
const BRANCHES: [char; 12] = [
    '子', '丑', '寅', '卯', '辰', '巳', '午', '未', '申', '酉', '戌', '亥',
];
/// The 24 solar terms by apparent ecliptic longitude, starting at 0° (春分).
const SOLAR_TERMS: [&str; 24] = [
    "春分", "清明", "谷雨", "立夏", "小满", "芒种", "夏至", "小暑", "大暑", "立秋", "处暑", "白露",
    "秋分", "寒露", "霜降", "立冬", "小雪", "大雪", "冬至", "小寒", "大寒", "立春", "雨水", "惊蛰",
];

/// Julian Day of the Unix epoch (1970-01-01T00:00:00Z).
const UNIX_EPOCH_JD: f64 = 2_440_587.5;
/// 立春 (Start of Spring) apparent solar longitude — the 干支 year boundary.
const LICHUN_LONGITUDE: f64 = 315.0;

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
    /// The Sun's apparent ecliptic longitude (degrees, `[0, 360)`) at the
    /// instant — from the stem-branch ephemeris; meridian-independent.
    pub solar_longitude_deg: f64,
    /// The current solar term (节气) implied by `solar_longitude_deg`.
    pub solar_term: String,
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
/// when given, shifts only the hour pillar; the lunar date, the solar terms, and
/// the year/month/day pillars are unaffected by it.
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
    let (year, month, day) = (i32::from(dt.year()), dt.month() as u8, dt.day() as u8);
    let civil_hour = i64::from(dt.hour());
    let civil_minute = i64::from(dt.minute());

    // --- Solar ephemeris (stem-branch): apparent longitude → solar terms ------
    #[allow(clippy::cast_precision_loss)]
    let jd_ut = instant.0 as f64 / 1e9 / 86_400.0 + UNIX_EPOCH_JD;
    let jde_tt = jd_ut + delta_t_for_year(f64::from(year)) / 86_400.0;
    let lambda = normalize_deg(solar_ecliptic_state(jde_tt).apparent_longitude_degrees);
    let solar_term = SOLAR_TERMS[((lambda / 15.0).floor() as usize) % 24].to_string();

    // --- Year pillar: flips at 立春 (315°). Jan is always before it; in Feb the
    // longitude disambiguates; Mar..Dec are after. (Meridian month only picks the
    // Jan-vs-Dec branch — both have λ < 315.) ---------------------------------
    let pillar_year = if month == 1 || (month == 2 && lambda < LICHUN_LONGITUDE) {
        year - 1
    } else {
        year
    };
    let year_stem = (pillar_year - 1984).rem_euclid(10) as usize;
    let year_pillar = pillar(year_stem, (pillar_year - 1984).rem_euclid(12) as usize);

    // --- Month pillar: the 节 sector (every 30° from 立春=315° → 寅月), stem via
    // 五虎遁 from the year stem. -----------------------------------------------
    let sector = (((lambda - LICHUN_LONGITUDE).rem_euclid(360.0)) / 30.0).floor() as usize;
    let month_branch = (2 + sector) % 12;
    let first_month_stem = (year_stem * 2 + 2) % 10; // 寅月 stem (五虎遁)
    let month_pillar = pillar((first_month_stem + sector) % 10, month_branch);

    // --- Day pillar: civil Julian-day number, anchored so 2020-01-25 = 丁卯; the
    // late-子 (23:00) civil hour rolls the displayed day forward. --------------
    let base_jdn = noon_jdn(year, month, day);
    let day_jdn = base_jdn + i64::from(civil_hour == 23);
    let day_cycle = (day_jdn + 49).rem_euclid(60);
    let day_pillar = pillar((day_cycle % 10) as usize, (day_cycle % 12) as usize);

    // --- Hour pillar: 時辰 from the (optionally longitude-corrected) hour, stem
    // via 五鼠遁 from that hour's day stem. The lunar date / other pillars keep
    // civil meridian time; only this pillar follows true solar time. ----------
    let ref_lon = f64::from(offset.seconds()) / 3600.0 * 15.0;
    let (eff_hour, solar_note) = match longitude {
        Some(lon) => {
            let corr_min = ((lon - ref_lon) * 4.0).round() as i64;
            let total = (civil_hour * 60 + civil_minute + corr_min).rem_euclid(24 * 60);
            (
                total / 60,
                format!(
                    "hour pillar uses local MEAN solar time (longitude {lon:.4}°E vs meridian {ref_lon:.1}°E, {corr_min:+} min); equation of time NOT applied, and the day/year/month pillars keep civil meridian time"
                ),
            )
        }
        None => (
            civil_hour,
            "hour pillar uses civil time at the meridian (no longitude → true solar time not applied)".to_string(),
        ),
    };
    let hour_branch = ((eff_hour + 1) / 2).rem_euclid(12) as usize;
    let hour_day_cycle = (base_jdn + i64::from(eff_hour == 23) + 49).rem_euclid(60);
    let hour_stem = (hour_day_cycle as usize % 10 % 5 * 2 + hour_branch) % 10;
    let hour_pillar = pillar(hour_stem, hour_branch);

    // --- Lunar (moon) calendar date: stem-branch (中氣 leap-month rule). The
    // upstream conversion panics outside its supported range; guard it into a
    // loud error so a far-out instant degrades gracefully, never crashes. ------
    let civil = CivilDate {
        year,
        month: u32::from(month),
        day: u32::from(day),
    };
    let lunar = std::panic::catch_unwind(AssertUnwindSafe(|| gregorian_to_lunisolar(civil)))
        .map_err(|_| {
            ChronoError::Render(format!(
            "lunisolar conversion is outside the supported range for {year}-{month:02}-{day:02}"
        ))
        })?;

    let assumptions = vec![
        format!(
            "Chinese reading computed for the {ref_lon:.1}°E meridian (UTC offset {} h); a different tradition (e.g. Vietnam UTC+7, Korea UTC+9) can differ by a day or a leap month",
            offset.seconds() / 3600
        ),
        "year pillar via 立春 and month pillar via the 12 节 (apparent solar longitude); the lunar DATE uses the 正月初一 new-year boundary (中氣 leap-month rule), so the year pillar and lunar year may differ near 立春".to_string(),
        solar_note,
    ];

    Ok(LunisolarReading {
        lunar_year: lunar.year,
        lunar_month: lunar.month as u8,
        lunar_day: lunar.day as u8,
        is_leap_month: lunar.is_leap_month,
        year_pillar,
        month_pillar,
        day_pillar,
        hour_pillar,
        solar_longitude_deg: lambda,
        solar_term,
        civil_local: instant
            .render(zone)
            .unwrap_or_else(|| "<out of civil range>".to_string()),
        assumptions,
    })
}

/// A two-character 干支 string from stem index (0..=9) and branch index (0..=11).
fn pillar(stem: usize, branch: usize) -> String {
    let mut s = String::with_capacity(6);
    s.push(STEMS[stem % 10]);
    s.push(BRANCHES[branch % 12]);
    s
}

/// Normalise degrees into `[0, 360)`.
fn normalize_deg(deg: f64) -> f64 {
    deg.rem_euclid(360.0)
}

/// The integer (noon) Julian Day Number of a civil date, via stem-branch's
/// `jd_from_ymd` (JD at 00:00 UT) + 0.5. Day boundary at noon, matching the
/// day-pillar anchor (2020-01-25 = 丁卯).
fn noon_jdn(year: i32, month: u8, day: u8) -> i64 {
    (jd_from_ymd(year, u32::from(month), u32::from(day)) + 0.5).floor() as i64
}
