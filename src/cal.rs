//! `timeglyph cal` — a forensics-grade calendar. This module is the pure data
//! builder: [`build_day`] computes the civil + timezone facts of a date (ISO
//! week, day-of-year, Julian Day Number, Modified JD, Unix midnight, weekday,
//! per-day UTC offset, DST fold/gap, and — behind the `leap` feature — leap-second
//! days and GPS week) with zero I/O, so it is fully testable and serialisable.
//! Alternative calendars, the moon/season visual layer, rendering, and the CLI
//! live in sibling modules built on top of this.

use crate::{ChronoError, Encoding, RenderZone};
use jiff::civil::Date;

/// Which day a week grid starts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeekStart {
    /// ISO default: Monday-first.
    Monday,
    /// Sunday-first (US convention).
    Sunday,
}

/// A month laid out as a grid of weeks over its [`CalDay`]s.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CalMonth {
    /// Calendar year.
    pub year: i16,
    /// Calendar month, 1–12.
    pub month: i8,
    /// The render zone, as a display label.
    pub zone_label: String,
    /// Rows of 7 cells; each cell is an index into [`days`](Self::days) or `None`
    /// for padding before/after the month.
    pub weeks: Vec<Vec<Option<usize>>>,
    /// The days of the month, in order (day N is `days[N-1]`).
    pub days: Vec<CalDay>,
}

/// A forensically-significant marker falling on a calendar day: a timestamp
/// format's epoch, or a rollover of a fixed-width time representation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Artifact {
    /// `"epoch"` (a format's zero point) or `"rollover"` (a width limit).
    pub kind: String,
    /// The format / rollover identifier (e.g. `filetime`, `unix_i32`).
    pub name: String,
    /// The exact UTC instant, RFC 3339.
    pub at_utc: String,
    /// The primary-source citation for the epoch/limit.
    pub citation: String,
}

/// A rollover of a fixed-width time representation. The instant is *derived* from
/// the structural limit (e.g. `i32::MAX` seconds), never a hardcoded date.
struct Rollover {
    name: &'static str,
    unix_second: i64,
    citation: &'static str,
}

/// Genuine domain discontinuities not derivable from a format's epoch — each the
/// documented limit of a fixed-width representation, cited to its spec.
const ROLLOVERS: &[Rollover] = &[
    Rollover {
        name: "unix_i32",
        unix_second: i32::MAX as i64,
        citation: "POSIX time_t, 32-bit signed (Year 2038)",
    },
    Rollover {
        name: "unix_u32",
        unix_second: u32::MAX as i64,
        citation: "time_t, 32-bit unsigned",
    },
];

/// The Chinese lunisolar date and 干支 pillars for a day (from the `lunisolar`
/// module's stem-branch ephemeris), computed at the day's noon in the render zone.
#[cfg(feature = "lunisolar")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChineseDate {
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
    /// Hour pillar (時柱), computed at the day's noon (午時).
    pub hour_pillar: String,
    /// The solar term (節氣) in effect.
    pub solar_term: String,
    /// The Sun's apparent ecliptic longitude (degrees) at the reference instant.
    pub solar_longitude_deg: f64,
}

/// The Hebrew calendar date for a day (from ICU4X).
#[cfg(feature = "altcal")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct HebrewDate {
    /// Anno Mundi year.
    pub year: i32,
    /// Month ordinal, 1-based (leap years insert Adar I).
    pub month: u8,
    /// Day of month.
    pub day: u8,
    /// ICU month code (`M01`..`M12`, `M05L` for a leap month) — unambiguous.
    pub month_code: String,
}

/// The Islamic (tabular civil, type II / Friday epoch) date for a day (ICU4X).
#[cfg(feature = "altcal")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct IslamicDate {
    /// Anno Hegirae year.
    pub year: i32,
    /// Month ordinal, 1-based.
    pub month: u8,
    /// Day of month.
    pub day: u8,
}

/// Hemisphere for mapping a solar longitude to a season name.
#[cfg(feature = "lunisolar")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hemisphere {
    /// Northern hemisphere (the March equinox opens spring).
    North,
    /// Southern hemisphere (the March equinox opens autumn).
    South,
}

/// The season a solar longitude falls in, for the given hemisphere. The Sun's
/// apparent longitude names the astronomical *event* (0° = March equinox); which
/// *season* that opens depends on hemisphere — a December solstice is austral
/// summer.
#[cfg(feature = "lunisolar")]
#[must_use]
pub fn season_for(solar_longitude_deg: f64, hemisphere: Hemisphere) -> &'static str {
    // North: [0,90)=spring [90,180)=summer [180,270)=autumn [270,360)=winter.
    let north = ["spring", "summer", "autumn", "winter"];
    let idx = (solar_longitude_deg.rem_euclid(360.0) / 90.0).floor() as usize % 4;
    match hemisphere {
        Hemisphere::North => north[idx],
        Hemisphere::South => north[(idx + 2) % 4],
    }
}

/// An astronomical season boundary — the instant the Sun's apparent longitude
/// reaches a cardinal value (0/90/180/270°).
#[cfg(feature = "lunisolar")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeasonMarker {
    /// The Sun's apparent ecliptic longitude at the boundary (0/90/180/270).
    pub solar_longitude_deg: f64,
    /// The UTC instant of the boundary, RFC 3339.
    pub instant_utc: String,
    /// The Chinese solar term (節氣) naming this boundary (春分/夏至/秋分/冬至).
    pub term_name: String,
}

/// The four astronomical season boundaries of `year` (March equinox → December
/// solstice), from the stem-branch solar-term solver (JPL-validated upstream).
/// Any boundary the solver cannot locate is omitted.
#[cfg(feature = "lunisolar")]
#[must_use]
pub fn season_markers(year: i16) -> Vec<SeasonMarker> {
    // (longitude, month to start the search in).
    const CARDINALS: [(f64, u32); 4] = [(0.0, 2), (90.0, 5), (180.0, 8), (270.0, 11)];
    let mut out = Vec::with_capacity(4);
    for (lon, start_month) in CARDINALS {
        if let Some(jd_ut) = stem_branch::find_solar_term_moment(lon, i32::from(year), start_month)
        {
            #[allow(clippy::cast_possible_truncation)]
            let unix = ((jd_ut - 2_440_587.5) * 86_400.0).round() as i64;
            if let Ok(ts) = jiff::Timestamp::from_second(unix) {
                out.push(SeasonMarker {
                    solar_longitude_deg: lon,
                    instant_utc: ts.to_string(),
                    term_name: stem_branch::solar_term_for_longitude(lon).to_string(),
                });
            }
        }
    }
    out
}

/// The eight moon-phase names, indexed by [`MoonInfo::phase_index`].
#[cfg(feature = "lunisolar")]
pub const PHASE_NAMES: [&str; 8] = [
    "New Moon",
    "Waxing Crescent",
    "First Quarter",
    "Waxing Gibbous",
    "Full Moon",
    "Waning Gibbous",
    "Last Quarter",
    "Waning Crescent",
];

/// The moon's phase geometry for a day (from the stem-branch ephemeris, Meeus
/// ch. 48), computed at the day's noon in the render zone.
#[cfg(feature = "lunisolar")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct MoonInfo {
    /// 8-phase bucket, 0 = new .. 4 = full .. 7 = waning crescent.
    pub phase_index: u8,
    /// Human phase name (see [`PHASE_NAMES`]).
    pub phase_name: String,
    /// Sun→Moon elongation, degrees `[0, 360)` (0 = new, 180 = full).
    pub elongation_deg: f64,
    /// Phase angle *i* (Sun–Moon–Earth), degrees `[0, 180]`.
    pub phase_angle_deg: f64,
    /// Illuminated fraction of the disc, `0.0`–`1.0`.
    pub illuminated_fraction: f64,
    /// `true` while waxing (new → full).
    pub waxing: bool,
}

/// A DST transition occurring within a calendar day.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DstTransition {
    /// `"gap"` (spring-forward, wall times skipped) or `"fold"` (fall-back, wall
    /// times repeated).
    pub kind: String,
    /// The UTC instant of the transition, RFC 3339.
    pub at_utc: String,
}

/// The civil + timezone facts of a single calendar day, serialised verbatim into
/// the machine (`--json`) view.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CalDay {
    /// ISO date `YYYY-MM-DD`.
    pub date: String,
    /// Lowercase English weekday name (`monday`..`sunday`).
    pub weekday: String,
    /// ISO-8601 week-numbering year (differs from the calendar year at Dec/Jan edges).
    pub iso_year: i16,
    /// ISO-8601 week number, 1–53.
    pub iso_week: i8,
    /// ISO-8601 weekday, 1 = Monday .. 7 = Sunday.
    pub iso_weekday: i8,
    /// Day of the year, 1–366.
    pub day_of_year: i16,
    /// Days in this year (365 or 366).
    pub days_in_year: i16,
    /// Julian Day Number (integer) of this date.
    pub jdn: i64,
    /// Modified Julian Day = `jdn − 2400001` (MJD epoch 1858-11-17).
    pub mjd: i64,
    /// Unix seconds at `00:00:00Z` of this date.
    pub unix_utc_midnight: i64,
    /// UTC offset (seconds) in effect at the start of the day in the render zone.
    pub offset_start_seconds: i32,
    /// UTC offset (seconds) in effect at the end of the day (i.e. at next midnight).
    pub offset_end_seconds: i32,
    /// Elapsed real seconds of the wall-clock day in the render zone (86400 on an
    /// ordinary day; 23 h / 25 h / etc. across a DST transition).
    pub wall_day_seconds: i64,
    /// The DST transition within the day, if any.
    pub dst_transition: Option<DstTransition>,
    /// Forensically-significant markers on this day (format epochs, rollovers).
    pub artifacts: Vec<Artifact>,
    /// Chinese lunisolar date + 干支 pillars, at the day's noon in the render zone.
    #[cfg(feature = "lunisolar")]
    pub alt_chinese: Option<ChineseDate>,
    /// Moon phase geometry at the day's noon in the render zone.
    #[cfg(feature = "lunisolar")]
    pub moon: Option<MoonInfo>,
    /// The Sun's apparent ecliptic longitude (degrees, `[0, 360)`) at the day's
    /// noon — the hemisphere-neutral season fact ([`season_for`] maps it to a name).
    #[cfg(feature = "lunisolar")]
    pub solar_longitude_deg: Option<f64>,
    /// Hebrew calendar date (civil-date based).
    #[cfg(feature = "altcal")]
    pub alt_hebrew: Option<HebrewDate>,
    /// Islamic (tabular civil) calendar date.
    #[cfg(feature = "altcal")]
    pub alt_islamic: Option<IslamicDate>,
    /// Leap seconds inserted (`+1`) / deleted (`-1`) during this UTC day.
    #[cfg(feature = "leap")]
    pub leap_second: i8,
    /// Length of the UTC day in seconds (`86400 + leap_second`).
    #[cfg(feature = "leap")]
    pub utc_day_seconds: i64,
    /// `true` if this UTC day is within ±12 h of a leap second (cloud-smear window).
    #[cfg(feature = "leap")]
    pub in_leap_smear_window: bool,
    /// GPS week number containing this day's 00:00 UTC instant.
    #[cfg(feature = "leap")]
    pub gps_week: i64,
}

/// Julian Day Number of the Unix epoch (1970-01-01).
const JDN_UNIX_EPOCH: i64 = 2_440_588;
/// `JDN − MJD` offset (the MJD epoch 1858-11-17 has JDN 2400001).
const MJD_OFFSET: i64 = 2_400_001;

/// The jiff time zone for a render zone.
fn zone_to_tz(zone: &RenderZone) -> jiff::tz::TimeZone {
    match zone {
        RenderZone::Utc => jiff::tz::TimeZone::UTC,
        RenderZone::Fixed(offset) => offset.to_time_zone(),
        RenderZone::Named(tz) => tz.clone(),
    }
}

/// Forensic markers falling on `date`: every registry format whose epoch lands on
/// this day (derived from the catalog, so a new format appears for free), plus any
/// cited fixed-width rollover.
fn artifacts_on(date: Date) -> Vec<Artifact> {
    let mut out = Vec::new();
    for f in crate::registry::FORMATS.iter() {
        let epoch_ns = match f.encoding {
            Encoding::LinearInt { epoch_ns, .. }
            | Encoding::LinearFloat { epoch_ns, .. }
            | Encoding::Embedded { epoch_ns, .. } => epoch_ns,
            Encoding::Packed(_) => continue, // packed civil formats have no epoch
        };
        // Skip epochs outside the representable range up front, so the timestamp
        // below is always valid (no dead error arm).
        let Ok(ts) = jiff::Timestamp::from_nanosecond(epoch_ns) else {
            continue;
        };
        if ts.to_zoned(jiff::tz::TimeZone::UTC).date() == date {
            out.push(Artifact {
                kind: "epoch".to_string(),
                name: f.id.to_string(),
                at_utc: ts.to_string(),
                citation: f.citation.to_string(),
            });
        }
    }
    for r in ROLLOVERS {
        let ts = jiff::Timestamp::from_second(r.unix_second);
        if let Ok(ts) = ts {
            if ts.to_zoned(jiff::tz::TimeZone::UTC).date() == date {
                out.push(Artifact {
                    kind: "rollover".to_string(),
                    name: r.name.to_string(),
                    at_utc: ts.to_string(),
                    citation: r.citation.to_string(),
                });
            }
        }
    }
    out
}

/// The first instant of `date` in `tz` (handles a midnight DST gap by moving to
/// the first valid instant, per jiff's compatible disambiguation).
fn first_instant(date: Date, tz: &jiff::tz::TimeZone) -> Result<jiff::Timestamp, ChronoError> {
    date.at(0, 0, 0, 0)
        .to_zoned(tz.clone())
        .map(|z| z.timestamp())
        // cov:unreachable: to_zoned uses compatible disambiguation, so midnight of
        // a valid civil date always resolves (a gap moves forward, never errors).
        .map_err(|e| ChronoError::Render(e.to_string()))
}

/// The Chinese lunisolar overlay for `date`, computed at the day's noon in `zone`.
#[cfg(feature = "lunisolar")]
fn chinese_on(date: Date, zone: &RenderZone) -> Option<ChineseDate> {
    let tz = zone_to_tz(zone);
    let noon = date
        .at(12, 0, 0, 0)
        .to_zoned(tz)
        .ok()?
        .timestamp()
        .as_nanosecond();
    let r = crate::lunisolar::render(crate::PosixNs(noon), zone, None).ok()?;
    Some(ChineseDate {
        lunar_year: r.lunar_year,
        lunar_month: r.lunar_month,
        lunar_day: r.lunar_day,
        is_leap_month: r.is_leap_month,
        year_pillar: r.year_pillar,
        month_pillar: r.month_pillar,
        day_pillar: r.day_pillar,
        hour_pillar: r.hour_pillar,
        solar_term: r.solar_term,
        solar_longitude_deg: r.solar_longitude_deg,
    })
}

/// The Hebrew and Islamic (tabular civil) overlays for `date`, via ICU4X.
#[cfg(feature = "altcal")]
fn altcal_on(date: Date) -> (Option<HebrewDate>, Option<IslamicDate>) {
    let Ok(iso) = icu_calendar::Date::try_new_iso(
        i32::from(date.year()),
        date.month() as u8,
        date.day() as u8,
    ) else {
        return (None, None);
    };
    let h = iso.to_calendar(icu_calendar::cal::Hebrew);
    let hebrew = HebrewDate {
        year: h.era_year().year,
        month: h.month().ordinal,
        day: h.day_of_month().0,
        month_code: h.month().to_input().code().to_string(),
    };
    let cal = icu_calendar::cal::Hijri::new_tabular(
        icu_calendar::cal::HijriTabularLeapYears::TypeII,
        icu_calendar::cal::HijriTabularEpoch::Friday,
    );
    let i = iso.to_calendar(cal);
    let islamic = IslamicDate {
        year: i.era_year().year,
        month: i.month().ordinal,
        day: i.day_of_month().0,
    };
    (Some(hebrew), Some(islamic))
}

/// The moon phase overlay for `date`, computed at the day's noon in `zone`.
#[cfg(feature = "lunisolar")]
fn moon_on(date: Date, zone: &RenderZone) -> Option<MoonInfo> {
    let tz = zone_to_tz(zone);
    let noon_ns = date
        .at(12, 0, 0, 0)
        .to_zoned(tz)
        .ok()?
        .timestamp()
        .as_nanosecond();
    #[allow(clippy::cast_precision_loss)]
    let jd_ut = noon_ns as f64 / 1e9 / 86_400.0 + 2_440_587.5;
    let jde_tt = jd_ut + stem_branch::delta_t_for_year(f64::from(date.year())) / 86_400.0;
    let p = stem_branch::moon_phase(jde_tt);
    // 8 buckets centred on the cardinal phases: [-22.5°, +22.5°) around each.
    let phase_index = (((p.elongation_deg + 22.5) / 45.0).floor() as i64).rem_euclid(8) as u8;
    Some(MoonInfo {
        phase_index,
        phase_name: PHASE_NAMES[phase_index as usize].to_string(),
        elongation_deg: p.elongation_deg,
        phase_angle_deg: p.phase_angle_deg,
        illuminated_fraction: p.illuminated_fraction,
        waxing: p.waxing,
    })
}

/// The Sun's apparent ecliptic longitude at `date`'s noon in `zone` (degrees).
#[cfg(feature = "lunisolar")]
fn solar_longitude_on(date: Date, zone: &RenderZone) -> Option<f64> {
    let tz = zone_to_tz(zone);
    let noon_ns = date
        .at(12, 0, 0, 0)
        .to_zoned(tz)
        .ok()?
        .timestamp()
        .as_nanosecond();
    #[allow(clippy::cast_precision_loss)]
    let jd_ut = noon_ns as f64 / 1e9 / 86_400.0 + 2_440_587.5;
    let jde_tt = jd_ut + stem_branch::delta_t_for_year(f64::from(date.year())) / 86_400.0;
    Some(
        stem_branch::solar_ecliptic_state(jde_tt)
            .apparent_longitude_degrees
            .rem_euclid(360.0),
    )
}

/// Build the civil + timezone facts of `date` in `zone`. Pure; never panics.
///
/// # Errors
/// Returns [`ChronoError`] only if the date is at the edge of the representable
/// range (never for an ordinary calendar date).
pub fn build_day(date: Date, zone: &RenderZone) -> Result<CalDay, ChronoError> {
    let unix_utc_midnight = date
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        // cov:unreachable: UTC midnight of a valid civil date is always in range.
        .map_err(|e| ChronoError::Render(e.to_string()))?
        .timestamp()
        .as_second();

    let jdn = JDN_UNIX_EPOCH + unix_utc_midnight.div_euclid(86_400);
    let iwd = date.iso_week_date();
    let iso_weekday = date.weekday().to_monday_one_offset();
    let weekday = match iso_weekday {
        1 => "monday",
        2 => "tuesday",
        3 => "wednesday",
        4 => "thursday",
        5 => "friday",
        6 => "saturday",
        _ => "sunday",
    };

    // Timezone overlay.
    let tz = zone_to_tz(zone);
    let tomorrow = date
        .tomorrow()
        // cov:unreachable: only fails at the maximum representable civil date.
        .map_err(|e| ChronoError::Render(e.to_string()))?;
    let start = first_instant(date, &tz)?;
    let next = first_instant(tomorrow, &tz)?;
    let offset_start_seconds = tz.to_offset(start).seconds();
    let offset_end_seconds = tz.to_offset(next).seconds();
    let wall_day_seconds = next.as_second() - start.as_second();
    let dst_transition = tz.following(start).next().and_then(|t| {
        (t.timestamp() < next).then(|| {
            let after = t.offset().seconds();
            DstTransition {
                kind: if after > offset_start_seconds {
                    "gap"
                } else {
                    "fold"
                }
                .to_string(),
                at_utc: t.timestamp().to_string(),
            }
        })
    });

    #[cfg(feature = "altcal")]
    let (alt_hebrew, alt_islamic) = altcal_on(date);

    Ok(CalDay {
        date: date.to_string(),
        weekday: weekday.to_string(),
        iso_year: iwd.year(),
        iso_week: iwd.week(),
        iso_weekday,
        day_of_year: date.day_of_year(),
        days_in_year: date.days_in_year(),
        jdn,
        mjd: jdn - MJD_OFFSET,
        unix_utc_midnight,
        offset_start_seconds,
        offset_end_seconds,
        wall_day_seconds,
        dst_transition,
        artifacts: artifacts_on(date),
        #[cfg(feature = "lunisolar")]
        alt_chinese: chinese_on(date, zone),
        #[cfg(feature = "lunisolar")]
        moon: moon_on(date, zone),
        #[cfg(feature = "lunisolar")]
        solar_longitude_deg: solar_longitude_on(date, zone),
        #[cfg(feature = "altcal")]
        alt_hebrew,
        #[cfg(feature = "altcal")]
        alt_islamic,
        #[cfg(feature = "leap")]
        leap_second: crate::leap::leap_seconds_on_utc_day(unix_utc_midnight),
        #[cfg(feature = "leap")]
        utc_day_seconds: 86_400
            + i64::from(crate::leap::leap_seconds_on_utc_day(unix_utc_midnight)),
        // Centre the ±12 h smear probe on the day's noon, so the day's own
        // window is [midnight, next midnight] — which brackets a real leap second
        // (always at 23:59:60), flagging the leap day itself.
        #[cfg(feature = "leap")]
        in_leap_smear_window: crate::leap::within_leap_smear_window(unix_utc_midnight + 43_200),
        #[cfg(feature = "leap")]
        gps_week: crate::leap::gps_week(unix_utc_midnight),
    })
}

/// A display label for a render zone (`UTC`, a fixed offset, or an IANA name).
fn zone_label(zone: &RenderZone) -> String {
    match zone {
        RenderZone::Utc => "UTC".to_string(),
        RenderZone::Fixed(o) => o.to_string(),
        RenderZone::Named(tz) => tz.iana_name().unwrap_or("local").to_string(),
    }
}

/// Build a whole month as a week grid over its days. Pure; never panics.
///
/// # Errors
/// Returns [`ChronoError`] if `year`/`month` is not a valid month or a day is at
/// the edge of the representable range.
pub fn build_month(
    year: i16,
    month: i8,
    zone: &RenderZone,
    week_start: WeekStart,
) -> Result<CalMonth, ChronoError> {
    let first = Date::new(year, month, 1).map_err(|e| ChronoError::Render(e.to_string()))?;
    let n = first.days_in_month();
    let mut days = Vec::with_capacity(n as usize);
    for d in 1..=n {
        // cov:unreachable: d ranges over days_in_month of a validated month, so
        // every Date::new here is in range (the first-of-month check already ran).
        let date = Date::new(year, month, d).map_err(|e| ChronoError::Render(e.to_string()))?;
        days.push(build_day(date, zone)?);
    }

    // Leading pad: how many blank cells before day 1, given the week start.
    let lead = match week_start {
        WeekStart::Monday => i32::from(first.weekday().to_monday_zero_offset()),
        WeekStart::Sunday => i32::from(first.weekday().to_sunday_zero_offset()),
    };
    let mut cells: Vec<Option<usize>> = (0..lead).map(|_| None).collect();
    cells.extend((0..days.len()).map(Some));
    while !cells.len().is_multiple_of(7) {
        cells.push(None);
    }
    let weeks: Vec<Vec<Option<usize>>> = cells.chunks(7).map(<[_]>::to_vec).collect();

    Ok(CalMonth {
        year,
        month,
        zone_label: zone_label(zone),
        weeks,
        days,
    })
}
