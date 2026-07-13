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
        let wk = week.iter().flatten().next().map_or_else(
            || "   ".to_string(),
            |&i| format!("W{:02}", m.days[i].iso_week),
        );
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
    out
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
    for a in &d.artifacts {
        let _ = writeln!(
            out,
            "  {} {} @ {} ({})",
            a.kind, a.name, a.at_utc, a.citation
        );
    }
    out
}
