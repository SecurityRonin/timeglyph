//! Shared calendar-display formatters — the single source of truth for the
//! Chinese lunar-date / 干支 / solar-term strings, the Hebrew and Islamic month
//! names, and the 五行 (Five Element) assignment. Both the `cal` subcommand
//! (via [`crate::cal_render`]) and the `timeglyph-lens` overlay consume these, so
//! the two never drift. Pure string/char logic, no dependencies.

/// A Chinese numeral for a small count (1..=30): `一`…`十`, `十一`…`廿九`, `三十`
/// (no `初` prefix — for the days-into-term phrase).
#[must_use]
pub fn cn_numeral(n: u32) -> String {
    const D: [&str; 10] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    match n {
        1..=9 => D[n as usize].to_string(),
        10 => "十".to_string(),
        11..=19 => format!("十{}", D[(n - 10) as usize]),
        20 => "二十".to_string(),
        21..=29 => format!("廿{}", D[(n - 20) as usize]),
        30 => "三十".to_string(),
        other => other.to_string(),
    }
}

/// The Chinese lunar month name (`正月`..`十二月`, `閏`-prefixed for a leap month).
#[must_use]
pub fn lunar_month_cn(month: u8, is_leap: bool) -> String {
    const M: [&str; 12] = [
        "正月",
        "二月",
        "三月",
        "四月",
        "五月",
        "六月",
        "七月",
        "八月",
        "九月",
        "十月",
        "十一月",
        "十二月",
    ];
    let name = M
        .get((month as usize).wrapping_sub(1))
        .copied()
        .unwrap_or("?月");
    if is_leap {
        format!("閏{name}")
    } else {
        name.to_string()
    }
}

/// The Chinese lunar day name (`初一`..`三十`).
#[must_use]
pub fn lunar_day_cn(day: u8) -> String {
    const D: [&str; 11] = [
        "", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十",
    ];
    match day {
        1..=10 => format!("初{}", D[day as usize]),
        11..=19 => format!("十{}", D[(day - 10) as usize]),
        20 => "二十".to_string(),
        21..=29 => format!("廿{}", D[(day - 20) as usize]),
        30 => "三十".to_string(),
        _ => day.to_string(),
    }
}

/// The full lunar date in Chinese, e.g. `正月廿二日` (`閏` prefix for a leap month).
#[must_use]
pub fn lunar_date_cn(month: u8, day: u8, is_leap: bool) -> String {
    format!("{}{}日", lunar_month_cn(month, is_leap), lunar_day_cn(day))
}

/// The solar term as a period phrase: the bare term on its own day, else
/// `<term>後第<N>日` (`雨水後第七日`), so a day well past the term is not misread
/// as the term's exact day.
#[must_use]
pub fn solar_term_phrase(term: &str, days_into_term: u32) -> String {
    if days_into_term == 0 {
        term.to_string()
    } else {
        format!("{term}後第{}日", cn_numeral(days_into_term))
    }
}

/// The three lines of the four-pillar (四柱) block from the year/month/day/hour
/// pillars: heavenly stems, earthly branches, and the `年月日時` labels — one
/// stem/branch character per pillar.
#[must_use]
pub fn four_pillar_rows(year: &str, month: &str, day: &str, hour: &str) -> [String; 3] {
    let mut stems = String::new();
    let mut branches = String::new();
    for p in [year, month, day, hour] {
        let mut ch = p.chars();
        stems.push(ch.next().unwrap_or('?'));
        branches.push(ch.next().unwrap_or('?'));
    }
    [stems, branches, "年月日時".to_string()]
}

/// Islamic (Hijri) month name for a 1-based ordinal.
#[must_use]
pub fn islamic_month(ordinal: u8) -> &'static str {
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

/// Persian (Solar Hijri) month name for a 1-based ordinal.
#[must_use]
pub fn persian_month(ordinal: u8) -> &'static str {
    const M: [&str; 12] = [
        "Farvardin",
        "Ordibehesht",
        "Khordad",
        "Tir",
        "Mordad",
        "Shahrivar",
        "Mehr",
        "Aban",
        "Azar",
        "Dey",
        "Bahman",
        "Esfand",
    ];
    M.get((ordinal as usize).wrapping_sub(1))
        .copied()
        .unwrap_or("?")
}

/// The Japanese era name (kanji) for an ICU era code (`reiwa`, `heisei`, …). The
/// modern eras; an unknown/older code falls back to the code itself.
#[must_use]
pub fn japanese_era(code: &str) -> &'static str {
    match code {
        "reiwa" => "令和",
        "heisei" => "平成",
        "showa" => "昭和",
        "taisho" => "大正",
        "meiji" => "明治",
        _ => "?",
    }
}

/// A short Gregorian month name for a 1-based ordinal (for the Buddhist era, whose
/// months are the Gregorian ones).
#[must_use]
pub fn greg_month_abbr(ordinal: u8) -> &'static str {
    const M: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    M.get((ordinal as usize).wrapping_sub(1))
        .copied()
        .unwrap_or("?")
}

/// Hebrew month name for an ICU month code (`M01`..`M12`, `M05L` = Adar I).
#[must_use]
pub fn hebrew_month(code: &str) -> &'static str {
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

/// The 五行 (Five Element) a 天干 / 地支 character belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    Wood,
    Fire,
    Earth,
    Metal,
    Water,
}

/// The Five Element of a stem or branch character (甲乙→Wood, 丙丁→Fire, …;
/// 寅卯→Wood, 巳午→Fire, 申酉→Metal, 亥子→Water, 辰戌丑未→Earth). `None` for a
/// non-干支 character.
#[must_use]
pub fn five_element(ch: char) -> Option<Element> {
    use Element::{Earth, Fire, Metal, Water, Wood};
    Some(match ch {
        // 天干 (stems) + 地支 (branches) sharing each element.
        '甲' | '乙' | '寅' | '卯' => Wood,
        '丙' | '丁' | '巳' | '午' => Fire,
        '戊' | '己' | '辰' | '戌' | '丑' | '未' => Earth,
        '庚' | '辛' | '申' | '酉' => Metal,
        '壬' | '癸' | '亥' | '子' => Water,
        _ => return None,
    })
}
