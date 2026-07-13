//! Pure renderers for [`crate::cal`] — turn a [`CalMonth`] into a monospace text
//! calendar for the terminal. No I/O, no colour side effects here: the grid uses
//! spaces only (never box-drawing characters, which misalign across fonts), one
//! marker glyph per day, and an ISO-week gutter. Machine (`--json`) output is the
//! serde serialisation of `CalMonth`/`CalDay`, not produced here.

use crate::cal::{CalDay, CalMonth};
use jiff::civil::Date;

/// The marker glyph for a day (highest-precedence condition wins). All ASCII and
/// single-width so the grid stays aligned and legible without colour.
///
/// `*` today · `^` DST gap · `v` DST fold · `+` leap-second day · `e` format
/// epoch · `~` rollover · space otherwise.
#[must_use]
pub fn day_marker(day: &CalDay, today: Option<Date>) -> char {
    if today.is_some_and(|t| t.to_string() == day.date) {
        return '*';
    }
    if let Some(t) = &day.dst_transition {
        return if t.kind == "gap" { '^' } else { 'v' };
    }
    #[cfg(feature = "leap")]
    if day.leap_second != 0 {
        return '+';
    }
    if day.artifacts.iter().any(|a| a.kind == "epoch") {
        return 'e';
    }
    if day.artifacts.iter().any(|a| a.kind == "rollover") {
        return '~';
    }
    ' '
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Islamic month name for a 1-based ordinal.
#[cfg(feature = "altcal")]
fn islamic_month(ordinal: u8) -> &'static str {
    const M: [&str; 12] = [
        "Muharram",
        "Safar",
        "Rabi I",
        "Rabi II",
        "Jumada I",
        "Jumada II",
        "Rajab",
        "Shaban",
        "Ramadan",
        "Shawwal",
        "Dhu al-Qidah",
        "Dhu al-Hijjah",
    ];
    M.get((ordinal as usize).wrapping_sub(1))
        .copied()
        .unwrap_or("?")
}

/// Hebrew month name for an ICU month code (`M01`..`M12`, `M05L` = Adar I).
#[cfg(feature = "altcal")]
fn hebrew_month(code: &str) -> &'static str {
    match code {
        "M01" => "Tishrei",
        "M02" => "Heshvan",
        "M03" => "Kislev",
        "M04" => "Tevet",
        "M05" => "Shevat",
        "M05L" => "Adar I",
        "M06" => "Adar",
        "M07" => "Nisan",
        "M08" => "Iyar",
        "M09" => "Sivan",
        "M10" => "Tammuz",
        "M11" => "Av",
        "M12" => "Elul",
        _ => "?",
    }
}

/// The northern-hemisphere season name for a solar longitude.
#[cfg(feature = "lunisolar")]
fn season_name(solar_longitude_deg: f64) -> &'static str {
    crate::cal::season_for(solar_longitude_deg, crate::cal::Hemisphere::North)
}

/// Render a month as a monospace grid: a title line, an ISO-week gutter, weekday
/// headers, and one `%2d` day + marker per cell, followed by a legend.
#[must_use]
pub fn render_month_text(m: &CalMonth, today: Option<Date>) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let name = MONTHS[(m.month as usize).clamp(1, 12) - 1];
    let _ = writeln!(out, "{name} {}                {}\n", m.year, m.zone_label);
    out.push_str("      Mo  Tu  We  Th  Fr  Sa  Su\n");

    for week in &m.weeks {
        // ISO week gutter: from the first real day in the row.
        let wk = match week.iter().flatten().next() {
            Some(&i) => format!("W{:02}", m.days[i].iso_week),
            // cov:unreachable: build_month never emits an all-None week (each row
            // has at least one real day), so the empty-gutter arm is dead.
            None => "   ".to_string(),
        };
        let _ = write!(out, "{wk} ");
        for cell in week {
            match cell {
                Some(i) => {
                    let d = &m.days[*i];
                    let _ = write!(out, "{:>3}{}", *i + 1, day_marker(d, today));
                }
                None => out.push_str("    "),
            }
        }
        out.push('\n');
    }

    out.push_str("\n  * today   ^ DST gap   v DST fold   + leap second   e epoch   ~ rollover\n");

    // Overlay footer: the month's alternative calendars, season, and moon phase,
    // taken from a representative mid-month day (an info panel, neofetch-style).
    #[cfg(feature = "lunisolar")]
    if let Some(mid) = m.days.get(m.days.len() / 2) {
        out.push('\n');
        if let Some(c) = &mid.alt_chinese {
            let _ = writeln!(
                out,
                "  {}年 · {} · lunar month {}",
                c.year_pillar, c.solar_term, c.lunar_month
            );
        }
        if let Some(lon) = mid.solar_longitude_deg {
            let _ = writeln!(out, "  season {}", season_name(lon));
        }
        if let Some(mo) = &mid.moon {
            let _ = writeln!(
                out,
                "  moon around mid-month: {} ({:.0}%)",
                mo.phase_name,
                mo.illuminated_fraction * 100.0
            );
        }
        #[cfg(feature = "altcal")]
        {
            let first = m.days.first();
            if let (Some(f), Some(l)) = (first, m.days.last()) {
                if let (Some(hf), Some(hl)) = (&f.alt_hebrew, &l.alt_hebrew) {
                    let _ = writeln!(
                        out,
                        "  hebrew {}–{} {}",
                        hebrew_month(&hf.month_code),
                        hebrew_month(&hl.month_code),
                        hl.year
                    );
                }
                if let (Some(isf), Some(isl)) = (&f.alt_islamic, &l.alt_islamic) {
                    let _ = writeln!(
                        out,
                        "  islamic {}–{} {}",
                        islamic_month(isf.month),
                        islamic_month(isl.month),
                        isl.year
                    );
                }
            }
        }
    }
    out
}

/// Append the day's alternative-calendar and season overlays (Chinese/干支,
/// Hebrew, Islamic, the season name + its scene tile) to the day card.
#[cfg(feature = "lunisolar")]
fn append_day_overlays(out: &mut String, d: &CalDay) {
    use std::fmt::Write as _;
    out.push('\n');
    if let Some(c) = &d.alt_chinese {
        let leap = if c.is_leap_month { " (leap)" } else { "" };
        let _ = writeln!(
            out,
            "  chinese   {}年 · lunar {}/{}{} · {} · {} pillar",
            c.year_pillar, c.lunar_month, c.lunar_day, leap, c.solar_term, c.day_pillar
        );
    }
    #[cfg(feature = "altcal")]
    if let Some(h) = &d.alt_hebrew {
        let _ = writeln!(
            out,
            "  hebrew    {} {} {}",
            h.day,
            hebrew_month(&h.month_code),
            h.year
        );
    }
    #[cfg(feature = "altcal")]
    if let Some(i) = &d.alt_islamic {
        let _ = writeln!(
            out,
            "  islamic   {} {} {}",
            i.day,
            islamic_month(i.month),
            i.year
        );
    }
    if let Some(lon) = d.solar_longitude_deg {
        let _ = writeln!(
            out,
            "  season    {} (N. hemisphere; solar longitude {lon:.1}deg)",
            season_name(lon)
        );
        // The seasonal scene tile (spring blossom / summer beach / autumn leaves /
        // winter snowman), keyed by the northern-hemisphere quarter.
        let idx = (lon.rem_euclid(360.0) / 90.0).floor() as u8 % 4;
        for line in crate::cal_art::season_tile(idx) {
            let _ = writeln!(out, "  {line}");
        }
    }
}

/// Render a single day's facts as a detail card (week/epoch systems, timezone,
/// and — where compiled in — leap/GPS and the alt-calendar/moon overlays).
#[must_use]
pub fn render_day_text(d: &CalDay) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "{}  {}", d.date, d.weekday);
    let _ = writeln!(
        out,
        "  iso {}-W{:02}-{}   doy {}/{}   jdn {}   mjd {}",
        d.iso_year, d.iso_week, d.iso_weekday, d.day_of_year, d.days_in_year, d.jdn, d.mjd
    );
    let _ = writeln!(
        out,
        "  unix midnight {}   offset {}s .. {}s   wall day {}s",
        d.unix_utc_midnight, d.offset_start_seconds, d.offset_end_seconds, d.wall_day_seconds
    );
    if let Some(t) = &d.dst_transition {
        let _ = writeln!(out, "  DST {} at {}", t.kind, t.at_utc);
    }
    #[cfg(feature = "leap")]
    {
        let _ = writeln!(
            out,
            "  leap {} (UTC day {}s)   gps week {}",
            d.leap_second, d.utc_day_seconds, d.gps_week
        );
    }
    #[cfg(feature = "lunisolar")]
    if let Some(mo) = &d.moon {
        out.push('\n');
        let disc = crate::cal_art::moon_art(mo.phase_index);
        for (i, line) in disc.iter().enumerate() {
            match i {
                0 => {
                    let _ = writeln!(out, "  {line:<18} {}", mo.phase_name);
                }
                1 => {
                    let _ = writeln!(
                        out,
                        "  {line:<18} {:.0}% illuminated",
                        mo.illuminated_fraction * 100.0
                    );
                }
                2 => {
                    let _ = writeln!(out, "  {line:<18} elongation {:.1}deg", mo.elongation_deg);
                }
                _ => {
                    let _ = writeln!(out, "  {line}");
                }
            }
        }
    }
    #[cfg(feature = "lunisolar")]
    append_day_overlays(&mut out, d);
    if !d.artifacts.is_empty() {
        out.push('\n');
    }
    for a in &d.artifacts {
        let _ = writeln!(
            out,
            "  {} {} @ {} ({})",
            a.kind, a.name, a.at_utc, a.citation
        );
    }
    out
}
