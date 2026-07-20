//! Pure renderers for [`crate::cal`] — turn a [`CalMonth`] into a monospace text
//! calendar for the terminal. No I/O, no colour side effects here: the grid uses
//! spaces only (never box-drawing characters, which misalign across fonts), one
//! marker glyph per day, and an ISO-week gutter. Machine (`--json`) output is the
//! serde serialisation of `CalMonth`/`CalDay`, not produced here.

use crate::cal::{CalDay, CalMonth};
use crate::cal_color::{self, ColorMode, Ink};
use jiff::civil::Date;

/// The palette [`Ink`] for a day's marker (`None` for `today`, which is reverse
/// video, and for an unmarked day). Mirrors [`day_marker`]'s precedence.
fn marker_ink(day: &CalDay) -> Option<Ink> {
    if let Some(t) = &day.dst_transition {
        return Some(if t.kind == "gap" {
            cal_color::GAP
        } else {
            cal_color::FOLD
        });
    }
    #[cfg(feature = "leap")]
    if day.leap_second != 0 {
        return Some(cal_color::LEAP);
    }
    if day.artifacts.iter().any(|a| a.kind == "epoch") {
        return Some(cal_color::EPOCH);
    }
    if day.artifacts.iter().any(|a| a.kind == "rollover") {
        return Some(cal_color::ROLLOVER);
    }
    None
}

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
/// headers, and one `%2d` day + marker per cell, followed by a legend. Markers are
/// coloured per `color` (today = reverse video); [`ColorMode::Mono`] is plain.
#[must_use]
pub fn render_month_text(m: &CalMonth, today: Option<Date>, color: ColorMode) -> String {
    use std::fmt::Write as _;
    let name = MONTHS[(m.month as usize).clamp(1, 12) - 1];
    // Build the calendar block (title + weekday header + week rows) as lines, then
    // optionally compose it right of the season scene tile.
    let mut grid: Vec<String> = Vec::new();
    grid.push(format!("{name} {}   {}", m.year, m.zone_label));
    grid.push(String::new());
    grid.push("      Mo  Tu  We  Th  Fr  Sa  Su".to_string());

    for week in &m.weeks {
        let mut out = String::new();
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
                    let glyph = day_marker(d, today);
                    let num = format!("{:>3}", *i + 1);
                    if today.is_some_and(|t| t.to_string() == d.date) {
                        // today: reverse-video the whole "NN*" cell.
                        let _ = write!(out, "{}", color.reverse(&format!("{num}{glyph}")));
                    } else if let Some(ink) = marker_ink(d) {
                        let _ = write!(out, "{num}{}", color.paint(ink, &glyph.to_string()));
                    } else {
                        let _ = write!(out, "{num}{glyph}");
                    }
                }
                None => out.push_str("    "),
            }
        }
        grid.push(out);
    }

    let mut result = grid.join("\n") + "\n";

    result
        .push_str("\n  * today   ^ DST gap   v DST fold   + leap second   e epoch   ~ rollover\n");

    // Overlay footer: the month's alternative calendars and moon, from a
    // representative mid-month day (facts beside the seasonal "logo" above).
    #[cfg(feature = "lunisolar")]
    if let Some(mid) = m.days.get(m.days.len() / 2) {
        result.push('\n');
        if let Some(c) = &mid.alt_chinese {
            let _ = writeln!(
                result,
                "  {}年 · {} · lunar month {}",
                c.year_pillar, c.solar_term, c.lunar_month
            );
        }
        if let Some(mo) = &mid.moon {
            let _ = writeln!(
                result,
                "  moon around mid-month: {} ({:.0}%)",
                mo.phase_name,
                mo.illuminated_fraction * 100.0
            );
        }
        // A compact alt-calendar hint (the mid-month year in each), full detail
        // lives in the single-day view.
        #[cfg(feature = "altcal")]
        if !mid.extra_calendars.is_empty() {
            let hint: Vec<String> = mid
                .extra_calendars
                .iter()
                .map(|e| format!("{} {}", e.name, e.year))
                .collect();
            let _ = writeln!(result, "  {}", hint.join(" · "));
        }
    }
    result
}

/// Append the day's alternative-calendar and season overlays (Chinese/干支,
/// Hebrew, Islamic, the season name + its scene tile) to the day card.
#[cfg(feature = "lunisolar")]
fn append_day_overlays(out: &mut String, d: &CalDay) {
    use std::fmt::Write as _;
    out.push('\n');
    if let Some(c) = &d.alt_chinese {
        use crate::calfmt;
        let _ = writeln!(
            out,
            "  農曆+干支暦 Lunisolar + Stem-Branch  {} · {}",
            calfmt::lunar_date_cn(c.lunar_month, c.lunar_day, c.is_leap_month),
            calfmt::solar_term_phrase(&c.solar_term, c.days_into_term)
        );
        // A blank line, then the four-pillar (四柱) block: stems over branches
        // over 年月日時 labels.
        out.push('\n');
        for row in calfmt::four_pillar_rows(
            &c.year_pillar,
            &c.month_pillar,
            &c.day_pillar,
            &c.hour_pillar,
        ) {
            let _ = writeln!(out, "            {row}");
        }
    }
    // Every alternative calendar, in one ordered list (中華民國 · Japanese ·
    // Buddhist · Hebrew · Islamic · Persian). The bilingual display names vary in
    // width, so each is followed by its formatted date (no fixed column).
    #[cfg(feature = "altcal")]
    {
        // Left-align the bilingual labels into one column (so the dates line up
        // too), measured by monospace display width — CJK is 2-wide, Devanagari
        // combining marks 0-wide, etc.
        use unicode_width::UnicodeWidthStr as _;
        let label_w = d
            .extra_calendars
            .iter()
            .map(|e| e.name.width())
            .max()
            .unwrap_or(0);
        for e in &d.extra_calendars {
            let pad = " ".repeat(label_w.saturating_sub(e.name.width()) + 2);
            let _ = writeln!(out, "  {}{}{}", e.name, pad, e.formatted);
        }
    }
    if let (Some(season), Some(lon)) = (&d.season, d.solar_longitude_deg) {
        // How far into the 90° season arc: early / mid / late (each ~30°).
        let stage = season_stage(lon);
        let cn = crate::calfmt::traditional_season(stage, season);
        let hemi = if d.southern_hemisphere { "S" } else { "N" };
        let _ = writeln!(
            out,
            "  season    {cn} {stage} {season} ({hemi}. hemisphere; solar longitude {lon:.1}deg)"
        );
    }
}

/// Which third of its 90° arc a solar longitude falls in: `early` / `mid` / `late`
/// (the season name is resolved separately, hemisphere-aware).
#[cfg(feature = "lunisolar")]
fn season_stage(solar_longitude_deg: f64) -> &'static str {
    let into = solar_longitude_deg.rem_euclid(90.0);
    if into < 30.0 {
        "early"
    } else if into < 60.0 {
        "mid"
    } else {
        "late"
    }
}

/// Paint a moon-disc art line: `@` lit (cream), `.` dark. Same visible width, so
/// callers pad *before* this. A no-op under [`ColorMode::Mono`].
fn paint_disc(line: &str, color: ColorMode) -> String {
    let mut s = String::new();
    for ch in line.chars() {
        match ch {
            '@' => s.push_str(&color.paint(cal_color::MOON_LIT, "@")),
            '.' => s.push_str(&color.paint(cal_color::MOON_DARK, ".")),
            other => s.push(other),
        }
    }
    s
}

/// Render a single day's detail card in plain text ([`ColorMode::Mono`]).
#[must_use]
pub fn render_day_text(d: &CalDay) -> String {
    render_day_text_with(d, ColorMode::Mono)
}

/// Render a single day's facts as a detail card (week/epoch systems, timezone,
/// and — where compiled in — leap/GPS and the alt-calendar/moon overlays), with
/// the moon disc and season tile tinted per `color`.
#[must_use]
pub fn render_day_text_with(d: &CalDay, color: ColorMode) -> String {
    use std::fmt::Write as _;
    let _ = &color; // used by the lunisolar-gated blocks below
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
            // Pad to 18 visible cols *before* painting, so ANSI bytes never skew it.
            let pad = " ".repeat(18_usize.saturating_sub(line.chars().count()));
            let art = paint_disc(line, color);
            match i {
                0 => {
                    let _ = writeln!(out, "  {art}{pad} {}", mo.phase_name);
                }
                1 => {
                    let _ = writeln!(
                        out,
                        "  {art}{pad} {:.0}% illuminated",
                        mo.illuminated_fraction * 100.0
                    );
                }
                2 => {
                    let _ = writeln!(out, "  {art}{pad} elongation {:.1}deg", mo.elongation_deg);
                }
                _ => {
                    let _ = writeln!(out, "  {art}");
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
