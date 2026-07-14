//! Shared calendar-display formatters (the DRY single source for cal + lens).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::calfmt::{
    cn_numeral, five_element, four_pillar_rows, hebrew_month, islamic_month, lunar_date_cn,
    lunar_day_cn, lunar_month_cn, solar_term_phrase, Element,
};

#[test]
fn chinese_numerals_and_lunar_dates() {
    assert_eq!(cn_numeral(7), "七");
    assert_eq!(cn_numeral(22), "廿二");
    assert_eq!(cn_numeral(30), "三十");
    assert_eq!(lunar_month_cn(1, false), "正月");
    assert_eq!(lunar_month_cn(4, true), "閏四月");
    assert_eq!(lunar_day_cn(1), "初一");
    assert_eq!(lunar_day_cn(22), "廿二");
    // The full lunar date, as shown on the cal day card (no "lunar" prefix).
    assert_eq!(lunar_date_cn(1, 22, false), "正月廿二日");
}

#[test]
fn solar_term_period_phrase() {
    assert_eq!(solar_term_phrase("冬至", 0), "冬至");
    assert_eq!(solar_term_phrase("雨水", 7), "雨水後第七日");
}

#[test]
fn four_pillars_split_into_stems_branches_labels() {
    // 年乙巳 月戊寅 日己未 時庚午 → stems 乙戊己庚 / branches 巳寅未午 / 年月日時.
    let rows = four_pillar_rows("乙巳", "戊寅", "己未", "庚午");
    assert_eq!(rows[0], "乙戊己庚");
    assert_eq!(rows[1], "巳寅未午");
    assert_eq!(rows[2], "年月日時");
}

#[test]
fn hebrew_and_islamic_month_names() {
    assert_eq!(hebrew_month("M01"), "Tishrei");
    assert_eq!(hebrew_month("M05L"), "Adar I");
    assert_eq!(hebrew_month("M12"), "Elul");
    assert_eq!(islamic_month(9), "Ramadan");
    assert_eq!(islamic_month(1), "Muharram");
}

#[test]
fn five_element_of_stems_and_branches() {
    assert_eq!(five_element('甲'), Some(Element::Wood));
    assert_eq!(five_element('午'), Some(Element::Fire));
    assert_eq!(five_element('酉'), Some(Element::Metal));
    assert_eq!(five_element('x'), None);
}
