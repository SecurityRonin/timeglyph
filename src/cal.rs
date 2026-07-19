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
    /// Calendar days since the solar term began (`0` = the term's own day).
    pub days_into_term: u32,
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

/// One additional calendar's date for a day — a generic, structured record
/// (Persian / Buddhist / Japanese) with a ready-to-display string.
#[cfg(feature = "altcal")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtraCal {
    /// A stable identifier (`roc`, `japanese`, `buddhist`, `hebrew`, `islamic`,
    /// `persian`) — the toggle key, decoupled from the display [`name`](Self::name)
    /// so renaming the label never breaks a consumer's visibility setting.
    pub key: String,
    /// Display name (native + English, e.g. `中華民國 Republic of China`).
    pub name: String,
    /// Year in that calendar (year-in-era for Japanese).
    pub year: i32,
    /// Month ordinal, 1-based.
    pub month: u8,
    /// Day of month.
    pub day: u8,
    /// A human-readable rendering (month names / era resolved).
    pub formatted: String,
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

/// The 114 southern-hemisphere IANA zones, generated from the tzdb `zone1970.tab`
/// coordinates (every zone whose latitude is south of the equator). A zone's
/// hemisphere is a geographic fact of its location, not a heuristic — this table
/// is the authoritative-source derivation of it.
#[cfg(feature = "lunisolar")]
const SOUTHERN_ZONES: &[&str] = &[
    "Africa/Blantyre",
    "Africa/Brazzaville",
    "Africa/Bujumbura",
    "Africa/Dar_es_Salaam",
    "Africa/Gaborone",
    "Africa/Harare",
    "Africa/Johannesburg",
    "Africa/Kigali",
    "Africa/Kinshasa",
    "Africa/Luanda",
    "Africa/Lubumbashi",
    "Africa/Lusaka",
    "Africa/Maputo",
    "Africa/Maseru",
    "Africa/Mbabane",
    "Africa/Nairobi",
    "Africa/Windhoek",
    "America/Araguaina",
    "America/Argentina/Buenos_Aires",
    "America/Argentina/Catamarca",
    "America/Argentina/Cordoba",
    "America/Argentina/Jujuy",
    "America/Argentina/La_Rioja",
    "America/Argentina/Mendoza",
    "America/Argentina/Rio_Gallegos",
    "America/Argentina/Salta",
    "America/Argentina/San_Juan",
    "America/Argentina/San_Luis",
    "America/Argentina/Tucuman",
    "America/Argentina/Ushuaia",
    "America/Asuncion",
    "America/Bahia",
    "America/Belem",
    "America/Campo_Grande",
    "America/Coyhaique",
    "America/Cuiaba",
    "America/Eirunepe",
    "America/Fortaleza",
    "America/Guayaquil",
    "America/La_Paz",
    "America/Lima",
    "America/Maceio",
    "America/Manaus",
    "America/Montevideo",
    "America/Noronha",
    "America/Porto_Velho",
    "America/Punta_Arenas",
    "America/Recife",
    "America/Rio_Branco",
    "America/Santarem",
    "America/Santiago",
    "America/Sao_Paulo",
    "Antarctica/Casey",
    "Antarctica/Davis",
    "Antarctica/DumontDUrville",
    "Antarctica/Macquarie",
    "Antarctica/Mawson",
    "Antarctica/McMurdo",
    "Antarctica/Palmer",
    "Antarctica/Rothera",
    "Antarctica/Syowa",
    "Antarctica/Troll",
    "Antarctica/Vostok",
    "Asia/Dili",
    "Asia/Jakarta",
    "Asia/Jayapura",
    "Asia/Makassar",
    "Atlantic/South_Georgia",
    "Atlantic/St_Helena",
    "Atlantic/Stanley",
    "Australia/Adelaide",
    "Australia/Brisbane",
    "Australia/Broken_Hill",
    "Australia/Darwin",
    "Australia/Eucla",
    "Australia/Hobart",
    "Australia/Lindeman",
    "Australia/Lord_Howe",
    "Australia/Melbourne",
    "Australia/Perth",
    "Australia/Sydney",
    "Indian/Antananarivo",
    "Indian/Chagos",
    "Indian/Christmas",
    "Indian/Cocos",
    "Indian/Comoro",
    "Indian/Kerguelen",
    "Indian/Mahe",
    "Indian/Mauritius",
    "Indian/Mayotte",
    "Indian/Reunion",
    "Pacific/Apia",
    "Pacific/Auckland",
    "Pacific/Bougainville",
    "Pacific/Chatham",
    "Pacific/Easter",
    "Pacific/Efate",
    "Pacific/Fakaofo",
    "Pacific/Fiji",
    "Pacific/Funafuti",
    "Pacific/Gambier",
    "Pacific/Guadalcanal",
    "Pacific/Kanton",
    "Pacific/Marquesas",
    "Pacific/Niue",
    "Pacific/Norfolk",
    "Pacific/Noumea",
    "Pacific/Pago_Pago",
    "Pacific/Pitcairn",
    "Pacific/Port_Moresby",
    "Pacific/Rarotonga",
    "Pacific/Tahiti",
    "Pacific/Tongatapu",
    "Pacific/Wallis",
];

/// The hemisphere for a render zone, derived from the IANA zone's latitude (tzdb
/// `zone1970.tab`). A named zone south of the equator is [`Hemisphere::South`];
/// UTC and fixed offsets carry no latitude, so they default to [`Hemisphere::North`].
#[cfg(feature = "lunisolar")]
#[must_use]
pub fn hemisphere_for(zone: &RenderZone) -> Hemisphere {
    if let RenderZone::Named(tz) = zone {
        if tz.iana_name().is_some_and(|n| SOUTHERN_ZONES.contains(&n)) {
            return Hemisphere::South;
        }
    }
    Hemisphere::North
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
    /// The season name (`spring`/`summer`/`autumn`/`winter`), resolved for the
    /// zone's hemisphere ([`hemisphere_for`]).
    #[cfg(feature = "lunisolar")]
    pub season: Option<String>,
    /// `true` if the render zone is in the southern hemisphere (the season was
    /// flipped from the northern-hemisphere event mapping).
    #[cfg(feature = "lunisolar")]
    pub southern_hemisphere: bool,
    /// Every alternative calendar, in display order (中華民國 · Japanese · Buddhist
    /// · Hebrew · Islamic · Persian).
    #[cfg(feature = "altcal")]
    pub extra_calendars: Vec<ExtraCal>,
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

/// The Chinese lunisolar overlay at reference instant `ref_ns` (ns since Unix),
/// at the `zone` meridian.
#[cfg(feature = "lunisolar")]
fn chinese_at(ref_ns: i128, zone: &RenderZone) -> Option<ChineseDate> {
    let r = crate::lunisolar::render(crate::PosixNs(ref_ns), zone, None).ok()?;
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
        days_into_term: r.days_into_term,
        solar_longitude_deg: r.solar_longitude_deg,
    })
}

/// The ICU4X ISO date for a jiff civil date, if representable.
#[cfg(feature = "altcal")]
fn icu_iso(date: Date) -> Option<icu_calendar::Date<icu_calendar::cal::Iso>> {
    icu_calendar::Date::try_new_iso(i32::from(date.year()), date.month() as u8, date.day() as u8)
        .ok()
}

/// The Hebrew calendar date for a civil date, via ICU4X. Reusable by any consumer
/// (the `cal` day card, the lens overlay).
#[cfg(feature = "altcal")]
#[must_use]
pub fn hebrew_date(date: Date) -> Option<HebrewDate> {
    let h = icu_iso(date)?.to_calendar(icu_calendar::cal::Hebrew);
    Some(HebrewDate {
        year: h.era_year().year,
        month: h.month().ordinal,
        day: h.day_of_month().0,
        month_code: h.month().to_input().code().to_string(),
    })
}

/// The Islamic (tabular civil, type II / Friday epoch) date for a civil date, via
/// ICU4X. Reusable by any consumer (the `cal` day card, the lens overlay).
#[cfg(feature = "altcal")]
#[must_use]
pub fn islamic_date(date: Date) -> Option<IslamicDate> {
    let cal = icu_calendar::cal::Hijri::new_tabular(
        icu_calendar::cal::HijriTabularLeapYears::TypeII,
        icu_calendar::cal::HijriTabularEpoch::Friday,
    );
    let i = icu_iso(date)?.to_calendar(cal);
    Some(IslamicDate {
        year: i.era_year().year,
        month: i.month().ordinal,
        day: i.day_of_month().0,
    })
}

/// All the alternative calendars for a civil date, via ICU4X, in the display
/// order 中華民國 · Japanese · Buddhist · Hebrew · Islamic · Persian — the single
/// ordered list shared by the `cal` day card and the lens overlay.
#[cfg(feature = "altcal")]
#[must_use]
pub fn extra_calendars(date: Date) -> Vec<ExtraCal> {
    use crate::calfmt::{
        greg_month_abbr, hebrew_month, islamic_month, japanese_era, persian_month,
    };
    let Some(iso) = icu_iso(date) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(6);

    // 1. 中華民國 (ROC / Minguo): Gregorian months, year = Gregorian − 1911.
    let r = iso.to_calendar(icu_calendar::cal::Roc);
    let (ry, rm, rd) = (r.era_year().year, r.month().ordinal, r.day_of_month().0);
    out.push(ExtraCal {
        key: "roc".to_string(),
        name: "中華民國 Republic of China".to_string(),
        year: ry,
        month: rm,
        day: rd,
        formatted: format!("{ry}年{rm}月{rd}日"),
    });

    // 2. Japanese era.
    let j = iso.to_calendar(icu_calendar::cal::Japanese::new());
    let jey = j.era_year();
    let (jy, jm, jd) = (jey.year, j.month().ordinal, j.day_of_month().0);
    out.push(ExtraCal {
        key: "japanese".to_string(),
        name: "和暦 Japanese".to_string(),
        year: jy,
        month: jm,
        day: jd,
        formatted: format!("{}{jy}年{jm}月{jd}日", japanese_era(jey.era.as_str())),
    });

    // 3. Buddhist (Gregorian months, +543 BE).
    let b = iso.to_calendar(icu_calendar::cal::Buddhist);
    let (by, bm, bd) = (b.era_year().year, b.month().ordinal, b.day_of_month().0);
    out.push(ExtraCal {
        key: "buddhist".to_string(),
        name: "बौद्ध संवत् Buddhist".to_string(),
        year: by,
        month: bm,
        day: bd,
        formatted: format!("{bd} {} {by} BE", greg_month_abbr(bm)),
    });

    // 4. Hebrew, 5. Islamic (from the typed converters, formatted for display).
    if let Some(h) = hebrew_date(date) {
        out.push(ExtraCal {
            key: "hebrew".to_string(),
            name: "לוח עברי Hebrew".to_string(),
            year: h.year,
            month: h.month,
            day: h.day,
            formatted: format!("{} {} {}", h.day, hebrew_month(&h.month_code), h.year),
        });
    }
    if let Some(i) = islamic_date(date) {
        out.push(ExtraCal {
            key: "islamic".to_string(),
            name: "هجري Islamic".to_string(),
            year: i.year,
            month: i.month,
            day: i.day,
            formatted: format!("{} {} {}", i.day, islamic_month(i.month), i.year),
        });
    }

    // 6. Persian (Solar Hijri).
    let p = iso.to_calendar(icu_calendar::cal::Persian);
    let (py, pm, pd) = (p.era_year().year, p.month().ordinal, p.day_of_month().0);
    out.push(ExtraCal {
        key: "persian".to_string(),
        name: "خورشیدی Persian".to_string(),
        year: py,
        month: pm,
        day: pd,
        formatted: format!("{pd} {} {py}", persian_month(pm)),
    });

    out
}

/// Every alternative calendar for the civil date of `instant` in `zone`, in
/// display order — the instant-based helper for the lens overlay.
#[cfg(feature = "altcal")]
#[must_use]
pub fn extra_calendars_at(instant: crate::PosixNs, zone: &RenderZone) -> Vec<ExtraCal> {
    let Ok(ts) = jiff::Timestamp::from_nanosecond(instant.0) else {
        return Vec::new();
    };
    extra_calendars(ts.to_zoned(zone_to_tz(zone)).date())
}

/// The Julian Ephemeris Day (TT) for a reference instant `ref_ns` (ns since Unix).
#[cfg(feature = "lunisolar")]
fn jde_tt_of(ref_ns: i128, year: i16) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let jd_ut = ref_ns as f64 / 1e9 / 86_400.0 + 2_440_587.5;
    jd_ut + stem_branch::delta_t_for_year(f64::from(year)) / 86_400.0
}

/// The moon phase overlay at reference instant `ref_ns` (ns since Unix).
#[cfg(feature = "lunisolar")]
fn moon_at(ref_ns: i128, year: i16) -> MoonInfo {
    let p = stem_branch::moon_phase(jde_tt_of(ref_ns, year));
    // 8 buckets centred on the cardinal phases: [-22.5°, +22.5°) around each.
    let phase_index = (((p.elongation_deg + 22.5) / 45.0).floor() as i64).rem_euclid(8) as u8;
    MoonInfo {
        phase_index,
        phase_name: PHASE_NAMES[phase_index as usize].to_string(),
        elongation_deg: p.elongation_deg,
        phase_angle_deg: p.phase_angle_deg,
        illuminated_fraction: p.illuminated_fraction,
        waxing: p.waxing,
    }
}

/// The Sun's apparent ecliptic longitude (degrees) at reference instant `ref_ns`.
#[cfg(feature = "lunisolar")]
fn solar_longitude_at(ref_ns: i128, year: i16) -> f64 {
    stem_branch::solar_ecliptic_state(jde_tt_of(ref_ns, year))
        .apparent_longitude_degrees
        .rem_euclid(360.0)
}

/// Build the civil + timezone facts of `date` in `zone`. Pure; never panics.
///
/// # Errors
/// Returns [`ChronoError`] only if the date is at the edge of the representable
/// range (never for an ordinary calendar date).
pub fn build_day(date: Date, zone: &RenderZone) -> Result<CalDay, ChronoError> {
    build_day_at(date.at(12, 0, 0, 0), zone)
}

/// Build a day card with the alternative-calendar / moon / solar overlays computed
/// at the specific civil datetime `dt` (so the 時柱 hour pillar and moon reflect the
/// actual time), not the day's noon. The civil/timezone/leap facts are still those
/// of `dt.date()`. Pure; never panics.
///
/// # Errors
/// Returns [`ChronoError`] only at the edge of the representable range.
pub fn build_day_at(dt: jiff::civil::DateTime, zone: &RenderZone) -> Result<CalDay, ChronoError> {
    let date = dt.date();
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

    // The reference instant for the astronomical/Chinese overlays (the given time
    // in the zone; compatible disambiguation resolves a DST gap, never errors).
    #[cfg(feature = "lunisolar")]
    let ref_ns: i128 = dt.to_zoned(tz.clone()).map_or_else(
        // cov:unreachable: compatible disambiguation resolves any wall time in range.
        |_| i128::from(unix_utc_midnight + 43_200) * 1_000_000_000,
        |z| z.timestamp().as_nanosecond(),
    );
    #[cfg(feature = "lunisolar")]
    let solar_lon = solar_longitude_at(ref_ns, date.year());
    #[cfg(feature = "lunisolar")]
    let hemisphere = hemisphere_for(zone);

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
        alt_chinese: chinese_at(ref_ns, zone),
        #[cfg(feature = "lunisolar")]
        moon: Some(moon_at(ref_ns, date.year())),
        #[cfg(feature = "lunisolar")]
        solar_longitude_deg: Some(solar_lon),
        #[cfg(feature = "lunisolar")]
        season: Some(season_for(solar_lon, hemisphere).to_string()),
        #[cfg(feature = "lunisolar")]
        southern_hemisphere: hemisphere == Hemisphere::South,
        #[cfg(feature = "altcal")]
        extra_calendars: extra_calendars(date),
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
