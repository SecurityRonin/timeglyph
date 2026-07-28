//! `timeglyph cal` — civil-exact week/epoch core (tier-1). Every expected value
//! is independently verifiable: ISO-8601 week dates and day-of-year cross-checked
//! against GNU/BSD `date +%G-W%V-%u`/`+%j` and Python `isocalendar()`; Julian Day
//! Numbers from USNO; the MJD epoch from IAU; Unix midnights arithmetic.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::float_cmp
)]

use jiff::civil::date;
use timeglyph::cal::{build_day, CalDay};
use timeglyph::RenderZone;

fn day(y: i16, m: i8, d: i8) -> CalDay {
    build_day(date(y, m, d), &RenderZone::Utc).unwrap()
}

fn zoned(y: i16, m: i8, d: i8, tz: &str) -> CalDay {
    let zone = RenderZone::parse(tz).unwrap();
    build_day(date(y, m, d), &zone).unwrap()
}

#[test]
fn jdn_mjd_unix_anchors() {
    // JDN 2000-01-01 = 2451545 (USNO); MJD = JDN − 2400001; Unix arithmetic.
    let a = day(2000, 1, 1);
    assert_eq!(a.jdn, 2_451_545);
    assert_eq!(a.mjd, 51_544);
    assert_eq!(a.unix_utc_midnight, 946_684_800);
    // MJD epoch 1858-11-17: JDN 2400001, MJD 0 (IAU / USNO).
    let e = day(1858, 11, 17);
    assert_eq!(e.jdn, 2_400_001);
    assert_eq!(e.mjd, 0);
    // Unix epoch 1970-01-01: JDN 2440588, Unix 0.
    let u = day(1970, 1, 1);
    assert_eq!(u.jdn, 2_440_588);
    assert_eq!(u.unix_utc_midnight, 0);
    // 2016-12-31: JDN 2457754 (= USNO JDN 2000-01-01 [2451545] + 6209 days).
    let c = day(2016, 12, 31);
    assert_eq!(c.jdn, 2_457_754);
    assert_eq!(c.mjd, 57_753);
    assert_eq!(c.unix_utc_midnight, 1_483_142_400);
}

#[test]
fn iso_week_edges() {
    // ISO 8601 canonical edge cases (vs `date +%G-W%V-%u`).
    let a = day(2000, 1, 1); // Sat → belongs to 1999-W52
    assert_eq!((a.iso_year, a.iso_week, a.iso_weekday), (1999, 52, 6));
    let b = day(2008, 12, 29); // Mon → 2009-W01
    assert_eq!((b.iso_year, b.iso_week, b.iso_weekday), (2009, 1, 1));
    let c = day(2010, 1, 3); // Sun → 2009-W53
    assert_eq!((c.iso_year, c.iso_week, c.iso_weekday), (2009, 53, 7));
    let d = day(2016, 12, 31); // Sat → 2016-W52
    assert_eq!((d.iso_year, d.iso_week, d.iso_weekday), (2016, 52, 6));
    let n = day(2026, 11, 1); // Sun → 2026-W44
    assert_eq!((n.iso_year, n.iso_week, n.iso_weekday), (2026, 44, 7));
}

#[test]
fn day_of_year_weekday_and_iso_string() {
    let c = day(2016, 12, 31);
    assert_eq!((c.day_of_year, c.days_in_year), (366, 366)); // leap year
    let n = day(2026, 11, 1);
    assert_eq!((n.day_of_year, n.days_in_year), (305, 365));
    assert_eq!(day(2000, 1, 1).weekday, "saturday");
    assert_eq!(day(1970, 1, 1).weekday, "thursday");
    assert_eq!(n.weekday, "sunday");
    assert_eq!(n.date, "2026-11-01");
}

// --- Cycle 2: leap-second days + GPS week (tier-1, hifitime IERS table) --------

#[cfg(feature = "leap")]
mod leapday {
    use timeglyph::leap::{gps_week, leap_seconds_on_utc_day};

    #[test]
    fn leap_second_days_from_iers_table() {
        // 2016-12-31 (unix midnight 1_483_142_400) and 2015-06-30 (1_435_622_400)
        // each carry an inserted leap second (IERS Bulletin C): cumulative TAI−UTC
        // rises by 1 across the UTC day (36→37, 35→36).
        assert_eq!(leap_seconds_on_utc_day(1_483_142_400), 1);
        assert_eq!(leap_seconds_on_utc_day(1_435_622_400), 1);
        // Ordinary days: no change.
        assert_eq!(leap_seconds_on_utc_day(1_483_056_000), 0); // 2016-12-30
        assert_eq!(leap_seconds_on_utc_day(1_483_228_800), 0); // 2017-01-01
    }

    #[test]
    fn gps_week_anchors() {
        assert_eq!(gps_week(315_964_800), 0); // 1980-01-06, GPS week 0
        assert_eq!(gps_week(1_554_595_200), 2048); // 2019-04-07 (post-rollover)
        assert_eq!(gps_week(1_793_491_200), 2443); // 2026-11-01
    }
}

// --- Cycle 3: TZ / DST overlay (tier-1 vs zdump / IANA tzdb) -------------------

#[test]
fn dst_fold_day_new_york() {
    // America/New_York 2026-11-01: fall-back FOLD at 06:00Z, -04:00 EDT -> -05:00
    // EST; the wall day is 25 h (90000 s). (zdump -v America/New_York)
    let d = zoned(2026, 11, 1, "America/New_York");
    assert_eq!(d.offset_start_seconds, -14400);
    assert_eq!(d.offset_end_seconds, -18000);
    assert_eq!(d.wall_day_seconds, 90_000);
    let t = d.dst_transition.expect("a transition on the fold day");
    assert_eq!(t.kind, "fold");
    assert_eq!(t.at_utc, "2026-11-01T06:00:00Z");
}

#[test]
fn dst_gap_day_new_york() {
    // 2026-03-08: spring-forward GAP at 07:00Z, -05:00 -> -04:00; wall day 23 h
    // (82800 s), 02:00-02:59 local never exists.
    let d = zoned(2026, 3, 8, "America/New_York");
    assert_eq!(d.offset_start_seconds, -18000);
    assert_eq!(d.offset_end_seconds, -14400);
    assert_eq!(d.wall_day_seconds, 82_800);
    assert_eq!(d.dst_transition.expect("gap").kind, "gap");
}

#[test]
fn dst_thirty_minute_fold_lord_howe() {
    // Australia/Lord_Howe 2026-04-05: 30-minute fall-back fold; wall day 24 h 30 m
    // (88200 s). (zdump -v Australia/Lord_Howe)
    let d = zoned(2026, 4, 5, "Australia/Lord_Howe");
    assert_eq!(d.wall_day_seconds, 88_200);
    assert_eq!(d.dst_transition.expect("fold").kind, "fold");
}

#[test]
fn ordinary_day_has_no_transition_and_full_wall_day() {
    let d = zoned(2026, 7, 1, "America/New_York");
    assert_eq!(d.wall_day_seconds, 86_400);
    assert!(d.dst_transition.is_none());
    assert_eq!(d.offset_start_seconds, -14400); // EDT
                                                // UTC day is always 86400 s except on a leap-second day.
    assert_eq!(day(2026, 7, 1).wall_day_seconds, 86_400);
}

#[cfg(feature = "leap")]
#[test]
fn leap_and_gps_fold_into_calday() {
    let c = day(2016, 12, 31);
    assert_eq!(c.leap_second, 1);
    assert_eq!(c.utc_day_seconds, 86_401);
    assert!(c.in_leap_smear_window);
    assert_eq!(c.gps_week, 1929);
    let n = day(2026, 7, 1);
    assert_eq!(n.leap_second, 0);
    assert_eq!(n.utc_day_seconds, 86_400);
    assert!(!n.in_leap_smear_window);
}

// --- Cycle 4: artifact ranges (registry epoch days + cited rollovers) ---------

fn has_artifact(d: &CalDay, kind: &str, name: &str) -> bool {
    d.artifacts.iter().any(|a| a.kind == kind && a.name == name)
}

#[test]
fn epoch_days_come_from_the_registry() {
    // Epoch instants are spec facts cited in forensicnomicon (tier-1).
    assert!(has_artifact(&day(1601, 1, 1), "epoch", "filetime"));
    assert!(has_artifact(&day(1601, 1, 1), "epoch", "webkit"));
    assert!(has_artifact(&day(1970, 1, 1), "epoch", "unix"));
    assert!(has_artifact(&day(1899, 12, 30), "epoch", "ole"));
    assert!(has_artifact(&day(1904, 1, 1), "epoch", "hfsplus"));
    assert!(has_artifact(&day(2001, 1, 1), "epoch", "cocoa"));
}

#[test]
fn rollovers_are_derived_from_structural_limits() {
    // 2038-01-19T03:14:07Z = i32::MAX seconds; 2106-02-07 = u32::MAX.
    assert!(has_artifact(&day(2038, 1, 19), "rollover", "unix_i32"));
    assert!(has_artifact(&day(2106, 2, 7), "rollover", "unix_u32"));
}

#[test]
fn ordinary_day_has_no_artifacts() {
    assert!(day(2026, 7, 1).artifacts.is_empty());
}

// --- Cycle 5a: Chinese/干支 overlay (reuse lunisolar; tier-1 vs cnlunar) -------

#[cfg(feature = "lunisolar")]
#[test]
fn chinese_overlay_matches_lunisolar() {
    // Any 2020 date after 立春 is year pillar 庚子; May is lunar month 4.
    let d = zoned(2020, 5, 31, "+08:00");
    let c = d.alt_chinese.expect("chinese overlay under lunisolar");
    assert_eq!(c.year_pillar, "庚子");
    assert_eq!(c.lunar_month, 4);
    // The overlay is exactly lunisolar::render at the day's noon in the zone.
    use timeglyph::{lunisolar, PosixNs, RenderZone};
    let zone = RenderZone::parse("+08:00").unwrap();
    let noon = jiff::civil::date(2020, 5, 31)
        .at(12, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::fixed(jiff::tz::offset(8)))
        .unwrap()
        .timestamp()
        .as_nanosecond();
    let r = lunisolar::render(PosixNs(noon), &zone, None).unwrap();
    assert_eq!(c.lunar_year, r.lunar_year);
    assert_eq!(c.lunar_day, r.lunar_day);
    assert_eq!(c.solar_term, r.solar_term);
}

// --- Cycle 5b: Hebrew + Islamic overlays (icu_calendar, feature=altcal) --------

#[cfg(feature = "altcal")]
#[test]
fn hebrew_and_islamic_overlays() {
    // 2007-09-13: Rosh Hashanah 5768 = 1 Tishrei 5768 (Hebrew); 1 Ramadan 1428
    // (Islamic tabular civil). Both independently verifiable (Hebcal / almanac).
    let cals = &day(2007, 9, 13).extra_calendars;
    let by = |k: &str| cals.iter().find(|e| e.key == k).unwrap();
    let h = by("hebrew");
    assert_eq!((h.year, h.month, h.day), (5768, 1, 1));
    let i = by("islamic");
    assert_eq!((i.year, i.month, i.day), (1428, 9, 1));
    // The unified list is in display order, ROC first, with a bilingual name.
    assert_eq!(cals[0].key, "roc");
    assert!(cals[0].name.contains("中華民國") && cals[0].name.contains("Republic of China"));
}

// --- Cycle 6a: moon phase overlay (stem-branch, feature=lunisolar) ------------

#[cfg(feature = "lunisolar")]
#[test]
fn moon_phase_overlay() {
    // 2024-09-18 was a full moon (02:34 UTC); at noon it is still ~full.
    let full = day(2024, 9, 18).moon.expect("moon overlay");
    assert_eq!(full.phase_index, 4);
    assert_eq!(full.phase_name, "Full Moon");
    assert!(
        full.illuminated_fraction > 0.98,
        "illum {}",
        full.illuminated_fraction
    );
    // 2024-04-08 was a new moon (eclipse, 18:21 UTC); near-zero illumination.
    let new = day(2024, 4, 8).moon.expect("moon overlay");
    assert_eq!(new.phase_index, 0);
    assert!(
        new.illuminated_fraction < 0.02,
        "illum {}",
        new.illuminated_fraction
    );
}

// --- Cycle 6b: season markers + hemisphere (stem-branch solar terms) ----------

#[cfg(feature = "lunisolar")]
mod seasons {
    use timeglyph::cal::{season_for, season_markers, Hemisphere};

    #[test]
    fn markers_land_on_the_known_solstice_equinox_dates_2026() {
        // Independently-known 2026 dates (almanac/USNO): Mar 20, Jun 21, Sep 23,
        // Dec 21. Longitudes 0/90/180/270; terms 春分/夏至/秋分/冬至.
        let m = season_markers(2026);
        assert_eq!(m[0].solar_longitude_deg, 0.0);
        assert!(
            m[0].instant_utc.starts_with("2026-03-20"),
            "{}",
            m[0].instant_utc
        );
        assert_eq!(m[0].term_name, "春分");
        assert!(
            m[1].instant_utc.starts_with("2026-06-21"),
            "{}",
            m[1].instant_utc
        );
        assert_eq!(m[1].term_name, "夏至");
        assert!(
            m[2].instant_utc.starts_with("2026-09-23"),
            "{}",
            m[2].instant_utc
        );
        assert!(
            m[3].instant_utc.starts_with("2026-12-21"),
            "{}",
            m[3].instant_utc
        );
        assert_eq!(m[3].term_name, "冬至");
    }

    #[test]
    fn hemisphere_maps_events_to_opposite_seasons() {
        // 0° opens spring in the north, autumn in the south; the December solstice
        // (270°) opens winter (north) / summer (south) — an austral beach.
        assert_eq!(season_for(0.0, Hemisphere::North), "spring");
        assert_eq!(season_for(0.0, Hemisphere::South), "autumn");
        assert_eq!(season_for(90.0, Hemisphere::North), "summer");
        assert_eq!(season_for(270.0, Hemisphere::North), "winter");
        assert_eq!(season_for(270.0, Hemisphere::South), "summer");
    }

    #[test]
    fn calday_carries_hemisphere_neutral_solar_longitude() {
        // Near the spring equinox the Sun's longitude is ~0°.
        use timeglyph::cal::build_day;
        use timeglyph::RenderZone;
        let d = build_day(jiff::civil::date(2026, 3, 20), &RenderZone::Utc).unwrap();
        let lon = d.solar_longitude_deg.expect("solar longitude");
        assert!(!(1.0..=359.0).contains(&lon), "lon {lon}");
    }
}

// --- Cycle 7: month grid + text/JSON rendering --------------------------------

#[test]
fn build_month_lays_out_the_grid() {
    use timeglyph::cal::{build_month, WeekStart};
    let m = build_month(2026, 7, &RenderZone::Utc, WeekStart::Monday).unwrap();
    assert_eq!(m.days.len(), 31);
    // 2026-07-01 is a Wednesday → Monday-first, day 1 sits in column index 2.
    assert_eq!(m.weeks[0][0], None);
    assert_eq!(m.weeks[0][1], None);
    assert_eq!(m.weeks[0][2], Some(0)); // day 1 → days[0]
    assert_eq!(m.weeks[0][6], Some(4)); // Sunday July 5 → days[4]
}

#[test]
fn render_month_text_shows_header_gutter_and_markers() {
    use timeglyph::cal::{build_month, WeekStart};
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::render_month_text;
    let m = build_month(2026, 7, &RenderZone::Utc, WeekStart::Monday).unwrap();
    let s = render_month_text(&m, None, ColorMode::Mono);
    assert!(s.contains("July 2026"), "{s}");
    assert!(s.contains("Mo") && s.contains("Su"));
    assert!(s.contains("W27")); // ISO week gutter
                                // No box-drawing characters (the alignment discipline).
    assert!(
        !s.chars().any(|c| ('\u{2500}'..='\u{257F}').contains(&c)),
        "box-drawing found"
    );
    // A leap-second day is flagged in the grid.
    #[cfg(feature = "leap")]
    {
        let dec = build_month(2016, 12, &RenderZone::Utc, WeekStart::Monday).unwrap();
        assert!(render_month_text(&dec, None, ColorMode::Mono).contains('+'));
    }
}

// --- Cycle 9: moon ASCII art (cal_art) ----------------------------------------

#[cfg(feature = "lunisolar")]
mod art {
    use timeglyph::cal_art::{moon_art, PHASE_GLYPH};

    #[test]
    fn moon_art_has_eight_discs_of_equal_height() {
        for idx in 0u8..8 {
            let a = moon_art(idx);
            assert_eq!(a.len(), 7, "phase {idx} art height");
        }
        // Full moon (index 4) is a fully lit disc; new moon (0) is dark.
        assert!(moon_art(4).iter().any(|l| l.contains('@')));
        assert!(!moon_art(0).iter().any(|l| l.contains('@')));
        // A single-width glyph per phase for the compact views.
        assert_eq!(PHASE_GLYPH.len(), 8);
    }

    #[test]
    fn day_card_shows_the_moon_disc_on_a_full_moon() {
        use timeglyph::cal::build_day;
        use timeglyph::cal_render::render_day_text;
        use timeglyph::RenderZone;
        let d = build_day(jiff::civil::date(2024, 9, 18), &RenderZone::Utc).unwrap();
        let card = render_day_text(&d);
        assert!(
            card.contains('@'),
            "full-moon day card should show a lit disc:\n{card}"
        );
    }

    #[test]
    fn day_card_capitalizes_the_weekday_name() {
        use timeglyph::cal::build_day;
        use timeglyph::cal_render::render_day_text;
        use timeglyph::RenderZone;
        // Human text view: a day name is a proper noun. The `weekday` field itself
        // stays lowercase for `--json` round-tripping; capitalization happens only
        // at the text render site.
        let card =
            render_day_text(&build_day(jiff::civil::date(2026, 7, 28), &RenderZone::Utc).unwrap());
        assert!(
            card.contains("Tuesday"),
            "weekday should be capitalized:\n{card}"
        );
        assert!(
            !card.contains("tuesday"),
            "weekday must not render lowercase:\n{card}"
        );
    }
}

// --- Cycle 10: season strip (year timeline) -----------------------------------

#[cfg(feature = "lunisolar")]
#[test]
fn season_strip_shows_boundaries_and_seasons() {
    use timeglyph::cal::{season_markers, Hemisphere};
    use timeglyph::cal_art::season_strip;
    let s = season_strip(2026, &season_markers(2026), Hemisphere::North);
    // Names the four seasons and the boundary months, no box-drawing.
    assert!(
        s.contains("Spring") && s.contains("Summer") && s.contains("Winter"),
        "{s}"
    );
    assert!(s.contains("2026"), "{s}");
    assert!(
        !s.chars().any(|c| ('\u{2500}'..='\u{257F}').contains(&c)),
        "box-drawing"
    );
    // Southern hemisphere flips: the December solstice opens summer.
    let south = season_strip(2026, &season_markers(2026), Hemisphere::South);
    assert!(south.contains("Summer"), "{south}");
}

// --- Coverage: exercise marker branches, season tiles, and error paths --------

#[test]
fn grid_markers_for_epoch_and_rollover_days() {
    use timeglyph::cal::{build_month, WeekStart};
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::render_month_text;
    // 1601-01 contains the FILETIME epoch → an 'e' marker.
    let jan1601 = build_month(1601, 1, &RenderZone::Utc, WeekStart::Sunday).unwrap();
    assert!(render_month_text(&jan1601, None, ColorMode::Mono).contains('e'));
    // 2038-01 contains the unix_i32 rollover → a '~' marker.
    let jan2038 = build_month(2038, 1, &RenderZone::Utc, WeekStart::Monday).unwrap();
    assert!(render_month_text(&jan2038, None, ColorMode::Mono).contains('~'));
    // Today marker lands when today is in view.
    let d = jiff::civil::date(2038, 1, 19);
    assert!(render_month_text(&jan2038, Some(d), ColorMode::Mono).contains('*'));
}

#[test]
fn build_month_rejects_an_invalid_month() {
    use timeglyph::cal::{build_month, WeekStart};
    assert!(build_month(2026, 13, &RenderZone::Utc, WeekStart::Monday).is_err());
}

// --- Cycle 11: surface the overlays in the human text views -------------------

#[cfg(all(feature = "lunisolar", feature = "altcal"))]
#[test]
fn day_card_shows_all_alternative_calendars_and_season() {
    use timeglyph::cal::build_day;
    use timeglyph::cal_render::render_day_text;
    // 2024-09-18: Chinese 白露 + four-pillar block, Hebrew 15 Elul 5784, Islamic
    // Rabi 1446.
    let card = render_day_text(&build_day(date(2024, 9, 18), &RenderZone::Utc).unwrap());
    assert!(
        card.contains("年月日時"),
        "four-pillar block missing:\n{card}"
    );
    // The 干支 line uses the lens-aligned label.
    assert!(
        card.contains("農曆+干支暦"),
        "aligned lunisolar label missing:\n{card}"
    );
    assert!(card.contains("白露"), "solar term missing:\n{card}");
    assert!(card.contains("5784"), "hebrew year missing:\n{card}");
    assert!(card.contains("Elul"), "hebrew month name missing:\n{card}");
    assert!(card.contains("1446"), "islamic year missing:\n{card}");
    // 2024-09-18 (solar longitude ~176°) is late summer — the autumn equinox
    // (180°) is Sept 22 — stated as a plain "late summer", no scene tile.
    assert!(
        card.contains("late summer"),
        "season stage missing:\n{card}"
    );
    let autumn = render_day_text(&build_day(date(2024, 11, 1), &RenderZone::Utc).unwrap());
    assert!(
        autumn.contains("autumn"),
        "autumn season missing:\n{autumn}"
    );
    let winter = render_day_text(&build_day(date(2025, 1, 15), &RenderZone::Utc).unwrap());
    assert!(
        winter.contains("winter"),
        "winter season missing:\n{winter}"
    );
    // The crude scene tiles are gone.
    assert!(
        !card.contains("-- O --") && !winter.contains("_===_"),
        "scene tile still drawn"
    );
}

#[cfg(feature = "lunisolar")]
#[test]
fn month_view_has_an_overlay_footer() {
    use timeglyph::cal::{build_month, WeekStart};
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::render_month_text;
    let m = build_month(2026, 7, &RenderZone::Utc, WeekStart::Monday).unwrap();
    let s = render_month_text(&m, None, ColorMode::Mono);
    // Single-column grid plus an info footer with the Chinese year — no side art.
    assert!(
        s.contains("年"),
        "chinese year pillar missing from month footer:\n{s}"
    );
    assert!(
        s.contains("Mo") && s.contains("Su"),
        "grid header missing:\n{s}"
    );
    assert!(
        !s.contains("-- O --"),
        "season scene tile should be gone:\n{s}"
    );
}

// --- Cycle 12: lunar Chinese date + four-pillar (四柱) block -------------------

#[cfg(feature = "lunisolar")]
#[test]
fn day_card_shows_lunar_chinese_date_and_four_pillars() {
    use timeglyph::cal::build_day;
    use timeglyph::cal_render::render_day_text;
    // 2025-02-19: 正月廿二日 · 雨水; 四柱 年乙巳 月戊寅 日己未 時庚午.
    let card = render_day_text(&build_day(date(2025, 2, 19), &RenderZone::Utc).unwrap());
    assert!(
        card.contains("正月廿二日"),
        "lunar Chinese date missing:\n{card}"
    );
    // The English "lunar" prefix was dropped — the Chinese date stands alone.
    assert!(
        !card.contains("lunar"),
        "the 'lunar' word should be gone:\n{card}"
    );
    assert!(card.contains("雨水"), "solar term missing:\n{card}");
    // Four-pillar block: stems / branches / 年月日時 labels (時 at noon = 午).
    assert!(card.contains("乙戊己庚"), "stems row missing:\n{card}");
    assert!(card.contains("巳寅未午"), "branches row missing:\n{card}");
    assert!(card.contains("年月日時"), "pillar labels missing:\n{card}");
}

// --- Cycle 13: solar term as a period phrase (雨水後第X日) ----------------------

#[cfg(feature = "lunisolar")]
#[test]
fn day_card_shows_solar_term_days_into_term() {
    use timeglyph::cal::build_day;
    use timeglyph::cal_render::render_day_text;
    // 2025-02-25 is 7 days into 雨水 → "雨水後第七日" (matching the lens phrasing).
    let card = render_day_text(&build_day(date(2025, 2, 25), &RenderZone::Utc).unwrap());
    assert!(
        card.contains("雨水後第七日"),
        "term-phrase missing:\n{card}"
    );
    // On the term's own day (days_into_term == 0) it is the bare term.
    // 2024-12-21 is 冬至's own day (day 0).
    let d0 = render_day_text(&build_day(date(2024, 12, 21), &RenderZone::Utc).unwrap());
    assert!(
        d0.contains("冬至") && !d0.contains("冬至後"),
        "bare term missing:\n{d0}"
    );
}

// --- Cycle 14: datetime input → the 時柱 reflects the real hour ----------------

#[cfg(feature = "lunisolar")]
#[test]
fn build_day_at_moves_only_the_hour_pillar() {
    use jiff::civil::datetime;
    use timeglyph::cal::build_day_at;
    let zone = RenderZone::parse("+08:00").unwrap();
    // Same civil date, four different hours (子/卯/未/亥時): the year/month/day
    // pillars are fixed; only the 時柱 changes (五鼠遁).
    let at = |h, m| {
        build_day_at(datetime(2025, 2, 19, h, m, 0, 0), &zone)
            .unwrap()
            .alt_chinese
            .unwrap()
    };
    let midnight = at(0, 30); // 子時
    let afternoon = at(14, 30); // 未時
    assert_eq!(
        midnight.day_pillar, afternoon.day_pillar,
        "day pillar must not move"
    );
    assert_eq!(midnight.hour_pillar, "甲子", "子時 hour pillar");
    assert_eq!(afternoon.hour_pillar, "辛未", "未時 hour pillar");
    // The default (noon) build agrees with an explicit noon instant.
    let noon = build_day_at(datetime(2025, 2, 19, 12, 0, 0, 0), &zone).unwrap();
    let default = build_day(date(2025, 2, 19), &zone).unwrap();
    assert_eq!(
        noon.alt_chinese.unwrap().hour_pillar,
        default.alt_chinese.unwrap().hour_pillar
    );
}

// --- Cycle 15: hemisphere derived from the zone (no --south flag) --------------

#[cfg(feature = "lunisolar")]
#[test]
fn hemisphere_and_season_are_derived_from_the_zone() {
    use timeglyph::cal::{hemisphere_for, Hemisphere};
    // A named southern zone resolves to South (from tzdb zone1970.tab); mid-January
    // is austral summer, boreal winter.
    let syd = RenderZone::parse("Australia/Sydney").unwrap();
    assert_eq!(hemisphere_for(&syd), Hemisphere::South);
    let s = build_day(date(2026, 1, 15), &syd).unwrap();
    assert_eq!(s.season.as_deref(), Some("summer"));
    assert!(s.southern_hemisphere);
    let ldn = RenderZone::parse("Europe/London").unwrap();
    assert_eq!(hemisphere_for(&ldn), Hemisphere::North);
    let n = build_day(date(2026, 1, 15), &ldn).unwrap();
    assert_eq!(n.season.as_deref(), Some("winter"));
    assert!(!n.southern_hemisphere);
    // UTC and a fixed offset carry no latitude → default North.
    assert_eq!(hemisphere_for(&RenderZone::Utc), Hemisphere::North);
    assert_eq!(
        hemisphere_for(&RenderZone::parse("+11:00").unwrap()),
        Hemisphere::North
    );
}

#[cfg(feature = "lunisolar")]
#[test]
fn day_card_season_follows_the_zone() {
    use timeglyph::cal_render::render_day_text;
    // Same date, opposite hemispheres: Sydney mid-January is summer, London winter.
    let syd = RenderZone::parse("Australia/Sydney").unwrap();
    let card = render_day_text(&build_day(date(2026, 1, 15), &syd).unwrap());
    assert!(card.contains("summer"), "sydney summer:\n{card}");
    let ldn = RenderZone::parse("Europe/London").unwrap();
    let cardn = render_day_text(&build_day(date(2026, 1, 15), &ldn).unwrap());
    assert!(cardn.contains("winter"), "london winter:\n{cardn}");
}

// --- extra_calendars_at: all six of an instant at a zone (shared with the lens) -

#[cfg(feature = "altcal")]
#[test]
fn extra_calendars_at_resolves_the_ordered_list_for_an_instant() {
    use timeglyph::cal::extra_calendars_at;
    use timeglyph::PosixNs;
    // 2007-09-13T12:00:00Z: Hebrew 1 Tishrei 5768, Islamic 1 Ramadan 1428.
    let ns = 1_189_684_800_i128 * 1_000_000_000;
    let cals = extra_calendars_at(PosixNs(ns), &RenderZone::Utc);
    assert_eq!(cals[0].key, "roc");
    let by = |k: &str| cals.iter().find(|e| e.key == k).unwrap();
    assert_eq!((by("hebrew").year, by("hebrew").day), (5768, 1));
    assert_eq!((by("islamic").year, by("islamic").month), (1428, 9));
}

// --- extra calendars: Persian / Buddhist / Japanese (icu, feature=altcal) ------

#[cfg(feature = "altcal")]
#[test]
fn extra_calendars_persian_buddhist_japanese() {
    // 2025-03-21 (Nowruz): Persian 1 Farvardin 1404; Buddhist 2568 (2025+543);
    // Japanese 令和7年 (Reiwa 7).
    let e = &day(2025, 3, 21).extra_calendars;
    let by_key = |k: &str| e.iter().find(|x| x.key == k).unwrap();
    let p = by_key("persian");
    assert_eq!((p.year, p.month, p.day), (1404, 1, 1));
    assert_eq!(p.formatted, "1 Farvardin 1404");
    let b = by_key("buddhist");
    assert_eq!(b.year, 2568);
    assert!(b.formatted.ends_with("BE"), "{}", b.formatted);
    let j = by_key("japanese");
    assert_eq!(j.year, 7);
    assert!(j.formatted.contains("令和"), "{}", j.formatted);
    // The year-only footer label carries the era/qualifier (a bare "8" would be
    // wrong for Japanese) — the month view shows this, not the raw year.
    assert_eq!(j.year_label, "令和7年");
    assert_eq!(b.year_label, "2568 BE");
    assert_eq!(p.year_label, "1404");
    // The month-view footer renders the era, not a bare number.
    use timeglyph::cal_color::ColorMode;
    let m =
        timeglyph::cal::build_month(2025, 3, &RenderZone::Utc, timeglyph::cal::WeekStart::Monday)
            .unwrap();
    let month = timeglyph::cal_render::render_month_text(&m, None, ColorMode::Mono);
    // The month footer renders the alt-calendars vertically, aligned like the day
    // card: each calendar on its OWN line, and the Japanese line carries the
    // era-qualified year (令和7年), not a bare number.
    let jp_line = month
        .lines()
        .find(|l| l.contains("和暦 Japanese"))
        .expect("month footer has a Japanese calendar line");
    assert!(jp_line.contains("令和7年"), "era-qualified year: {month}");
    assert!(
        !jp_line.contains("中華民國"),
        "footer is vertical — one calendar per line, not a · -joined row: {month}"
    );
    // A straddling calendar shows its month(s) + the Gregorian transition date:
    // in March 2025 the Hebrew calendar rolls Adar → Nisan on 2025-03-30.
    let heb_line = month
        .lines()
        .find(|l| l.contains("Hebrew"))
        .expect("month footer has a Hebrew calendar line");
    assert!(
        heb_line.contains("Adar 5785")
            && heb_line.contains("Nisan 5785")
            && heb_line.contains("from 2025-03-30"),
        "month transition with Gregorian date: {month}"
    );
    // Shown in the day card (bilingual name + the Japanese era value).
    let card = timeglyph::cal_render::render_day_text(&day(2025, 3, 21));
    assert!(card.contains("Persian") && card.contains("令和"), "{card}");
}
