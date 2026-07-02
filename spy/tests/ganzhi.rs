//! Tests for the 干支 / lunisolar view behind the overlay's opt-in expansion.
#![allow(clippy::unwrap_used)]

use timeglyph::{PosixNs, RenderZone};
use timeglyph_spy::ganzhi;

// 2000-01-01T00:00:00Z (Y2K), viewed at the China meridian (+08:00).
fn y2k() -> PosixNs {
    PosixNs(946_684_800_000_000_000)
}

#[test]
fn ganzhi_view_has_four_pillars_a_date_and_a_term() {
    let zone = RenderZone::parse("+08:00").unwrap();
    let v = ganzhi::ganzhi_view(y2k(), &zone, None).unwrap();
    // Each pillar is a two-character 干支 (stem + branch).
    for p in [
        &v.year_pillar,
        &v.month_pillar,
        &v.day_pillar,
        &v.hour_pillar,
    ] {
        assert_eq!(p.chars().count(), 2, "pillar must be 2 CJK chars: {p:?}");
    }
    assert!(!v.lunar_date.is_empty());
    assert!(!v.solar_term.is_empty());
    // The meridian/convention choices must be surfaced (a reading, not a verdict).
    assert!(!v.assumptions.is_empty());
}

#[test]
fn longitude_correction_leaves_date_and_ymd_pillars_untouched() {
    let zone = RenderZone::parse("+08:00").unwrap();
    let base = ganzhi::ganzhi_view(y2k(), &zone, None).unwrap();
    let corrected = ganzhi::ganzhi_view(y2k(), &zone, Some(121.5)).unwrap();
    // The optional longitude correction touches only the hour pillar; the lunar
    // date and the year/month/day pillars are invariant under it.
    assert_eq!(base.year_pillar, corrected.year_pillar);
    assert_eq!(base.month_pillar, corrected.month_pillar);
    assert_eq!(base.day_pillar, corrected.day_pillar);
    assert_eq!(base.lunar_date, corrected.lunar_date);
}

#[test]
fn lunar_date_uses_chinese_notation_not_gregorian_looking() {
    // 2020-01-01 UTC is lunar 己亥年十二月初七. It must render in Chinese month/day
    // notation — NOT "2019年 12月 7日", which reads like the Gregorian date
    // 2019-12-07 and confuses users.
    let inst = PosixNs(1_577_836_800_000_000_000); // 2020-01-01T00:00:00Z
    let v = ganzhi::ganzhi_view(inst, &RenderZone::Utc, None).unwrap();
    assert!(
        v.lunar_date.contains("十二月初七"),
        "expected Chinese lunar notation, got {:?}",
        v.lunar_date
    );
    assert!(
        !v.lunar_date.contains('日'),
        "must not use the Gregorian-looking 日 form: {:?}",
        v.lunar_date
    );
}

#[test]
fn parse_longitude_accepts_in_range_rejects_the_rest() {
    assert_eq!(ganzhi::parse_longitude("121.5"), Some(121.5));
    assert_eq!(ganzhi::parse_longitude("-74"), Some(-74.0));
    assert_eq!(ganzhi::parse_longitude(""), None); // empty → no correction
    assert_eq!(ganzhi::parse_longitude("999"), None); // out of ±180
    assert_eq!(ganzhi::parse_longitude("east"), None); // not a number
}
