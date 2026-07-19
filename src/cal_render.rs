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

/// The monospace display width of `c`: 2 for East-Asian wide / fullwidth glyphs
/// (CJK, Hangul, kana, fullwidth forms), 1 otherwise.
fn char_width(c: char) -> usize {
    let u = c as u32;
    if (0x1100..=0x115F).contains(&u)      // Hangul Jamo
        || (0x2E80..=0xA4CF).contains(&u)  // CJK radicals … Yi
        || (0xAC00..=0xD7A3).contains(&u)  // Hangul syllables
        || (0xF900..=0xFAFF).contains(&u)  // CJK compat ideographs
        || (0xFE30..=0xFE4F).contains(&u)  // CJK compat forms
        || (0xFF00..=0xFF60).contains(&u)  // fullwidth forms
        || (0xFFE0..=0xFFE6).contains(&u)
        || (0x20000..=0x3FFFD).contains(&u)
    // CJK extension planes
    {
        2
    } else {
        1
    }
}

/// The display width of a line, skipping ANSI SGR escapes (`\x1b[…m`) and counting
/// East-Asian wide glyphs as 2 columns.
fn visible_width(s: &str) -> usize {
    let mut w = 0;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until the SGR terminator 'm'.
            for e in chars.by_ref() {
                if e == 'm' {
                    break;
                }
            }
        } else {
            w += char_width(c);
        }
    }
    w
}

/// Compose `left` and `right` line-blocks side by side, top-aligned, padding each
/// left line to `left_w` visible columns (a neofetch-style art-beside-grid panel).
fn side_by_side(left: &[String], right: &[String], left_w: usize) -> String {
    let mut out = String::new();
    for i in 0..left.len().max(right.len()) {
        let l = left.get(i).map_or("", String::as_str);
        let r = right.get(i).map_or("", String::as_str);
        let pad = " ".repeat(left_w.saturating_sub(visible_width(l)));
        out.push_str(l);
        out.push_str(&pad);
        out.push_str("  ");
        out.push_str(r);
        out.push('\n');
    }
    out
}

/// Render a month as a monospace grid: a title line, an ISO-week gutter, weekday
/// headers, and one `%2d` day + marker per cell, followed by a legend. Markers are
/// coloured per `color` (today = reverse video); [`ColorMode::Mono`] is plain. When
/// a season is known, the season scene tile is placed to the left, neofetch-style.
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

    // Compose: the season scene tile (left "logo") beside the calendar grid.
    #[cfg(feature = "lunisolar")]
    let mut result = match m
        .days
        .get(m.days.len() / 2)
        .and_then(|d| d.season.as_deref())
    {
        Some(season) => {
            let ink = cal_color::season_ink(season);
            let art: Vec<String> = crate::cal_art::season_tile_for(season)
                .iter()
                .map(|l| color.paint(ink, l))
                .collect();
            side_by_side(&art, &grid, 16)
        }
        None => grid.join("\n") + "\n",
    };
    #[cfg(not(feature = "lunisolar"))]
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
fn append_day_overlays(out: &mut String, d: &CalDay, color: ColorMode) {
    use std::fmt::Write as _;
    out.push('\n');
    if let Some(c) = &d.alt_chinese {
        use crate::calfmt;
        let _ = writeln!(
            out,
            "  chinese   {} · {}",
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
    for e in &d.extra_calendars {
        let _ = writeln!(out, "  {}  {}", e.name, e.formatted);
    }
    if let (Some(season), Some(lon)) = (&d.season, d.solar_longitude_deg) {
        let hemi = if d.southern_hemisphere { "S" } else { "N" };
        let _ = writeln!(
            out,
            "  season    {season} ({hemi}. hemisphere; solar longitude {lon:.1}deg)"
        );
        // The seasonal scene tile (spring blossom / summer beach / autumn leaves /
        // winter snowman), keyed by the resolved season name and tinted its colour.
        let ink = cal_color::season_ink(season);
        for line in crate::cal_art::season_tile_for(season) {
            let _ = writeln!(out, "  {}", color.paint(ink, line));
        }
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
    append_day_overlays(&mut out, d, color);
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
