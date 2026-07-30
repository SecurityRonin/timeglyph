//! Line-level edges the happy-path suites miss: the branch and error arms that
//! ADR-0008's 100%-line gate requires be genuinely exercised rather than
//! annotated away. Provably-dead defensive arms carry `// cov:unreachable` in the
//! source instead; everything here is a reachable path with an asserted outcome.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use jiff::civil::date;
use timeglyph::{format, interpret, scan, DateStyle, PosixNs, RenderZone};

// ── scan ────────────────────────────────────────────────────────────────────

#[test]
fn weekday_names_all_seven_days() {
    // A full ISO week starting Monday 2026-07-27 (cross-checked against
    // `date -j -f %Y-%m-%d 2026-07-27 +%A` / Python `strftime('%A')`).
    for (iso, name) in [
        ("2026-07-27", "Monday"),
        ("2026-07-28", "Tuesday"),
        ("2026-07-29", "Wednesday"),
        ("2026-07-30", "Thursday"),
        ("2026-07-31", "Friday"),
        ("2026-08-01", "Saturday"),
        ("2026-08-02", "Sunday"),
    ] {
        assert_eq!(
            scan::weekday(&format!("{iso}T12:00:00Z")),
            Some(name),
            "{iso}"
        );
    }
}

#[test]
fn a_scan_past_the_size_cap_truncates_on_a_char_boundary() {
    // 3-byte characters, so the 1 MiB cap (1048576 ≡ 1 mod 3) lands *inside* a
    // character and the boundary walk must step back rather than slice mid-code-
    // point. The timestamp appended past the cap must not be seen.
    let mut text = "\u{20ac}".repeat(scan::MAX_SCAN_BYTES / 2);
    text.push_str(" 1577836800");
    assert!(
        scan::inspect_text(&text, 3, &RenderZone::Utc).is_empty(),
        "content beyond MAX_SCAN_BYTES is dropped"
    );
}

#[test]
fn a_hex_token_that_is_not_decodable_yields_no_readings() {
    // `0x`-prefixed tokens bypass the hex-candidate length floor, so an odd
    // number of hex digits reaches interpret_hex and fails there.
    let found = scan::inspect_text("value 0xabc here", 3, &RenderZone::Utc);
    assert!(
        !found.iter().any(|n| n.number == "0xabc"),
        "odd-length hex is not decoded: {found:?}"
    );
}

// ── calfmt ──────────────────────────────────────────────────────────────────

#[cfg(feature = "lunisolar")]
#[test]
fn chinese_numerals_cover_the_tens_and_the_out_of_range_tail() {
    use timeglyph::calfmt::{cn_numeral, lunar_day_cn};
    assert_eq!(cn_numeral(20), "二十");
    assert_eq!(cn_numeral(30), "三十");
    // Above 30 there is no lunar-day numeral, so the count is rendered verbatim.
    assert_eq!(cn_numeral(31), "31");
    assert_eq!(lunar_day_cn(20), "二十");
    assert_eq!(lunar_day_cn(30), "三十");
    assert_eq!(lunar_day_cn(31), "31");
}

#[cfg(feature = "altcal")]
#[test]
fn an_unknown_hebrew_month_code_renders_as_a_question_mark() {
    assert_eq!(timeglyph::calfmt::hebrew_month("M99"), "?");
}

#[cfg(feature = "lunisolar")]
#[test]
fn five_element_covers_earth_and_water_stems_and_branches() {
    use timeglyph::calfmt::{five_element, Element};
    assert_eq!(five_element('戊'), Some(Element::Earth));
    assert_eq!(five_element('丑'), Some(Element::Earth));
    assert_eq!(five_element('壬'), Some(Element::Water));
    assert_eq!(five_element('子'), Some(Element::Water));
    assert_eq!(five_element('A'), None);
}

// ── cal_color ───────────────────────────────────────────────────────────────

#[test]
fn reverse_video_is_emitted_for_every_colour_capability_but_mono() {
    use timeglyph::cal_color::ColorMode;
    for mode in [ColorMode::Truecolor, ColorMode::Ansi256, ColorMode::Ansi16] {
        assert_eq!(mode.reverse("x"), "\x1b[7mx\x1b[0m", "{mode:?}");
    }
    assert_eq!(ColorMode::Mono.reverse("x"), "x");
}

// ── datefmt ─────────────────────────────────────────────────────────────────

#[test]
fn a_naive_reading_keeps_its_style_minus_the_zone_token() {
    use timeglyph::datefmt::format_naive;
    let inst = PosixNs(1_577_836_800 * 1_000_000_000);
    assert_eq!(
        format_naive(inst, DateStyle::SpaceSeparated),
        "2020-01-01 00:00:00"
    );
    assert_eq!(
        format_naive(inst, DateStyle::Rfc2822),
        "Wed, 01 Jan 2020 00:00:00"
    );
}

#[test]
fn a_naive_reading_outside_the_civil_range_says_so() {
    use timeglyph::datefmt::format_naive;
    for style in [
        DateStyle::Iso8601,
        DateStyle::SpaceSeparated,
        DateStyle::Rfc2822,
        DateStyle::UsStyle,
    ] {
        assert_eq!(
            format_naive(PosixNs(i128::MAX), style),
            "<out of civil range>"
        );
    }
}

// ── localzone ───────────────────────────────────────────────────────────────

#[test]
fn a_naive_instant_outside_the_civil_range_resolves_to_itself() {
    use timeglyph::localzone::{resolve_local, LocalResolution};
    let zone = RenderZone::parse("America/New_York").unwrap();
    assert_eq!(
        resolve_local(PosixNs(i128::MAX), &zone),
        LocalResolution::Unique(PosixNs(i128::MAX))
    );
}

// ── lib: encode ─────────────────────────────────────────────────────────────

#[test]
fn a_float_only_format_refuses_integer_encoding() {
    // OLE automation date is a LinearFloat format: encode_int has no integer
    // representation to produce, so it errors rather than fabricating one.
    let f = format("ole").unwrap();
    let err = f
        .encode_int(PosixNs(1_577_836_800 * 1_000_000_000))
        .expect_err("a float format cannot encode to an integer");
    assert!(
        format!("{err}").contains("float-format encoded as integer"),
        "{err}"
    );
}

// ── registry: packed encoders reject out-of-field-range years ────────────────

#[test]
fn packed_encoders_reject_years_outside_their_bit_field() {
    let ns = |y: i16| -> PosixNs {
        PosixNs(
            date(y, 6, 15)
                .at(12, 0, 0, 0)
                .to_zoned(jiff::tz::TimeZone::UTC)
                .unwrap()
                .timestamp()
                .as_nanosecond(),
        )
    };
    // DTTM packs the year as an offset from 1900 in 9 bits → 1900..=2411.
    let err = format("dttm").unwrap().encode_int(ns(1899)).unwrap_err();
    assert!(format!("{err}").contains("DTTM year"), "{err}");
    // BitDate packs the year in 12 bits → 0..=4095; a negative year cannot fit.
    let err = format("bitdate").unwrap().encode_int(ns(-1)).unwrap_err();
    assert!(format!("{err}").contains("BitDate year"), "{err}");
    // Bitwise Decimal stores the year unsigned.
    let err = format("bitdec").unwrap().encode_int(ns(-1)).unwrap_err();
    assert!(format!("{err}").contains("Bitwise Decimal year"), "{err}");
}

#[test]
fn sql_server_tick_rounding_rolls_into_the_next_day() {
    // The 1/300 s tick grid: 23:59:59.999999999 rounds up past the day's last
    // tick (25_919_999), so the encoder must carry into the next day rather than
    // emit an out-of-range tick.
    let f = format("sqlserver").unwrap();
    let end_of_day = PosixNs(
        date(2020, 1, 1)
            .at(23, 59, 59, 999_999_999)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp()
            .as_nanosecond(),
    );
    let packed = f.encode_int(end_of_day).unwrap();
    let back = f.decode_int(packed).unwrap();
    assert_eq!(
        back.to_rfc3339().unwrap(),
        "2020-01-02T00:00:00Z",
        "the rounded-up tick rolls the day"
    );
}

// ── interpret: scoring context ──────────────────────────────────────────────

#[test]
fn an_observed_width_with_no_meaningful_byte_swap_still_scores() {
    // 2-byte and 16-byte widths have no 4/8-byte swap partner, so endian_match
    // falls back to "this order only" rather than comparing against a swap.
    for width in [2u8, 16u8] {
        let ctx = interpret::InterpretContext {
            observed_width_bytes: Some(width),
            endian: Some(interpret::Endian::Little),
            ..Default::default()
        };
        let cands = interpret::interpret_int_with_context(1_577_836_800, &ctx);
        assert!(!cands.is_empty(), "width {width}");
        assert!(
            cands[0]
                .components
                .iter()
                .any(|(n, _)| *n == "endian_match"),
            "width {width} still scores endian_match"
        );
    }
}

// ── interpret: JWT / base64url ──────────────────────────────────────────────

#[test]
fn a_token_with_four_parts_is_not_read_as_a_jwt() {
    // `{"iat":1577836800}` base64url-encoded, with a fourth segment appended.
    let payload = "eyJpYXQiOjE1Nzc4MzY4MDB9";
    assert!(
        interpret::interpret_string(&format!("h.{payload}.s"))
            .iter()
            .any(|c| c.format_id.contains("jwt")),
        "the three-part form IS read as a JWT"
    );
    assert!(
        !interpret::interpret_string(&format!("h.{payload}.s.extra"))
            .iter()
            .any(|c| c.format_id.contains("jwt")),
        "a fourth segment disqualifies it"
    );
}

#[test]
fn a_jwt_payload_that_is_not_base64url_is_not_decoded() {
    assert!(
        interpret::interpret_string("h.!!!not-b64!!!.s").is_empty(),
        "a non-base64url payload yields no JWT claims"
    );
}

#[test]
fn base64url_padding_and_the_url_safe_alphabet_decode() {
    // `{"iat":1577836800,"x":1}` → padded base64url, exercising the `=` stop.
    let padded = "eyJpYXQiOjE1Nzc4MzY4MDAsIngiOjF9";
    assert!(
        interpret::interpret_string(&format!("h.{padded}==.s"))
            .iter()
            .any(|c| c.format_id.contains("jwt")),
        "trailing padding is tolerated"
    );
    // `-` and `_` are the URL-safe substitutions for `+` and `/`: they must map
    // to their 6-bit values (the payload then fails JSON parsing, which is the
    // documented "not a JWT" outcome, not a decode error).
    assert!(
        !interpret::interpret_string("h.ab-_cdef.s")
            .iter()
            .any(|c| c.format_id.contains("jwt")),
        "a base64url payload that is not JSON is not a JWT"
    );
}

// ── interpret: string-format parsers reject malformed shapes ────────────────

#[test]
fn a_syslog_line_with_extra_fields_is_rejected() {
    let reference = PosixNs(1_577_836_800 * 1_000_000_000);
    assert!(interpret::parse_syslog_with_reference("Jan  2 03:04:05", reference).is_some());
    // A fourth time field, and a trailing token, each disqualify the shape.
    assert!(interpret::parse_syslog_with_reference("Jan  2 03:04:05:06", reference).is_none());
    assert!(interpret::parse_syslog_with_reference("Jan  2 03:04:05 host", reference).is_none());
}

#[test]
fn a_common_log_date_needs_a_numeric_offset_and_exactly_three_date_fields() {
    let is_clf = |s: &str| {
        interpret::interpret_string(s)
            .iter()
            .any(|c| c.format_id == "clf")
    };
    assert!(
        is_clf("[10/Oct/2000:13:55:36 -0700]"),
        "the canonical shape"
    );
    // A named zone instead of ±HHMM (no sign byte).
    assert!(!is_clf("[10/Oct/2000:13:55:36 GMT]"));
    // A sign but the wrong number of offset digits.
    assert!(!is_clf("[10/Oct/2000:13:55:36 +07]"));
    // A fourth `/`-separated date field.
    assert!(!is_clf("[10/Oct/2000/1:13:55:36 -0700]"));
    // A fourth `:`-separated time field.
    assert!(!is_clf("[10/Oct/2000:13:55:36:99 -0700]"));
}

#[test]
fn a_pdf_date_without_an_offset_is_read_as_utc() {
    let pdf = |s: &str| {
        interpret::interpret_string(s)
            .into_iter()
            .find(|c| c.format_id == "pdf_date")
            .and_then(|c| c.rendered)
    };
    // ISO 32000-1 §7.9.4 allows the offset to be omitted or given as `Z`.
    assert_eq!(
        pdf("D:20200101120000").as_deref(),
        Some("2020-01-01T12:00:00Z")
    );
    assert_eq!(
        pdf("D:20200101120000Z").as_deref(),
        Some("2020-01-01T12:00:00Z")
    );
    assert_eq!(
        pdf("D:20200101120000+05'30'").as_deref(),
        Some("2020-01-01T06:30:00Z")
    );
}

#[test]
fn a_dmtf_cim_datetime_needs_14_date_digits_6_frac_digits_and_a_3_digit_offset() {
    let cim = |s: &str| {
        interpret::interpret_string(s)
            .iter()
            .any(|c| c.format_id == "dmtf_cim")
    };
    assert!(cim("20200101120000.000000+000"), "the canonical shape");
    // `***` is DSP0004's "offset unknown", treated as UTC.
    assert!(cim("20200101120000.000000+***"), "unknown offset");
    // Too few date digits.
    assert!(!cim("202001011.000000+000"));
    // Wrong fractional-second width.
    assert!(!cim("20200101120000.123+000"));
    // A 4-digit offset is not the DSP0004 whole-minutes field.
    assert!(!cim("20200101120000.000000+0000"));
}

// ── csv enrichment ──────────────────────────────────────────────────────────

#[cfg(feature = "csv")]
mod csv_edges {
    use timeglyph::csv_enrich::{enrich, Conversion, EnrichOptions};
    use timeglyph::RenderZone;

    fn opts(conversions: Vec<Conversion>, auto: bool) -> EnrichOptions {
        EnrichOptions {
            conversions,
            auto,
            replace: false,
            zone: RenderZone::Utc,
        }
    }

    fn conv(column: &str, format: &str) -> Conversion {
        Conversion {
            column: column.to_string(),
            format: format.to_string(),
        }
    }

    #[test]
    fn auto_detection_leaves_an_explicitly_mapped_column_alone() {
        // Column `ts` is pinned to `unix`; auto must skip it (not re-detect it as
        // something else) while still considering the remaining columns.
        let csv = "ts,other\n1577836800,x\n1577836900,y\n1577837000,z\n";
        let out = enrich(csv, &opts(vec![conv("ts", "unix")], true)).unwrap();
        assert!(out.starts_with("ts,ts_unix,other\n"), "{out}");
        assert!(out.contains("2020-01-01T00:00:00Z"), "{out}");
    }

    #[test]
    fn a_cell_that_no_decoder_accepts_renders_empty() {
        // i64::MAX seconds overflows the nanosecond range for the integer path,
        // and `unix` is not a float format, so neither decoder yields an instant.
        let csv = "ts\n9223372036854775807\n";
        let out = enrich(csv, &opts(vec![conv("ts", "unix")], false)).unwrap();
        assert_eq!(out, "ts,ts_unix\n9223372036854775807,\n", "{out}");
    }

    #[test]
    fn auto_detection_skips_blank_cells_but_needs_at_least_one_value() {
        // Blank cells are skipped, so a column that is otherwise coherent is
        // still detected...
        let csv = "ts\n1577836800\n\n1577836900\n1577837000\n";
        let out = enrich(csv, &opts(vec![], true)).unwrap();
        assert!(out.starts_with("ts,ts_unix\n"), "{out}");
        // ...but an all-blank column has nothing to detect.
        let csv = "a,b\n,1\n,2\n";
        let out = enrich(csv, &opts(vec![], true)).unwrap();
        assert_eq!(out, "a,b\n,1\n,2\n", "{out}");
    }

    #[test]
    fn auto_detection_declines_a_sentinel_column() {
        // 0x7FFFFFFFFFFFFFFF is the AD `accountExpires` "never" marker: a
        // sentinel, not an instant, so the column is left untouched.
        let csv = "ts\n9223372036854775807\n9223372036854775807\n";
        let out = enrich(csv, &opts(vec![], true)).unwrap();
        assert_eq!(
            out, "ts\n9223372036854775807\n9223372036854775807\n",
            "{out}"
        );
    }

    #[test]
    fn auto_detection_declines_an_out_of_window_column() {
        // Above the magnitude floor but outside every format's plausible window
        // (the top reading is .NET ticks in year 1941), so the column is left
        // untouched rather than enriched with an implausible instant.
        let csv = "ts\n-900000000000000000\n-900000000000000001\n";
        let out = enrich(csv, &opts(vec![], true)).unwrap();
        assert_eq!(
            out, "ts\n-900000000000000000\n-900000000000000001\n",
            "{out}"
        );
    }
}

// ── cal: alternative calendars and zone labels ──────────────────────────────

#[cfg(feature = "altcal")]
#[test]
fn alternative_calendars_of_an_unrepresentable_instant_are_empty() {
    assert!(
        timeglyph::cal::extra_calendars_at(PosixNs(i128::MAX), &RenderZone::Utc).is_empty(),
        "an instant outside the civil range has no calendar date"
    );
}

#[test]
fn a_day_card_labels_a_fixed_offset_and_a_named_zone() {
    use timeglyph::cal::build_day;
    let fixed = build_day(date(2020, 6, 15), &RenderZone::parse("+05:30").unwrap()).unwrap();
    let named = build_day(date(2020, 6, 15), &RenderZone::parse("Asia/Tokyo").unwrap()).unwrap();
    // The zone reaches the rendered card, so the label arm for each variant runs.
    let fixed_text = timeglyph::cal_render::render_day_text(&fixed);
    let named_text = timeglyph::cal_render::render_day_text(&named);
    assert!(fixed_text.contains("2020-06-15"), "{fixed_text}");
    assert!(named_text.contains("2020-06-15"), "{named_text}");
    assert_eq!(fixed.offset_start_seconds, 19_800, "+05:30");
    assert_eq!(named.offset_start_seconds, 32_400, "JST");
}

// ── cal_render: DST markers and the artifact block ──────────────────────────

#[test]
fn a_dst_gap_and_fold_day_get_their_own_marker_and_card_line() {
    use timeglyph::cal::build_day;
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::{day_marker, render_day_text_with};
    let ny = RenderZone::parse("America/New_York").unwrap();
    // 2020-03-08 spring-forward (gap), 2020-11-01 fall-back (fold) — US DST.
    let gap = build_day(date(2020, 3, 8), &ny).unwrap();
    let fold = build_day(date(2020, 11, 1), &ny).unwrap();
    assert_eq!(day_marker(&gap, None), '^', "gap glyph");
    assert_eq!(day_marker(&fold, None), 'v', "fold glyph");
    for (d, kind) in [(&gap, "gap"), (&fold, "fold")] {
        let text = render_day_text_with(d, ColorMode::Truecolor);
        assert!(text.contains(&format!("DST {kind} at ")), "{text}");
    }
}

#[test]
fn a_month_grid_paints_the_dst_marker() {
    use timeglyph::cal::{build_month, WeekStart};
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::render_month_text;
    let ny = RenderZone::parse("America/New_York").unwrap();
    let m = build_month(2020, 3, &ny, WeekStart::Sunday).unwrap();
    let text = render_month_text(&m, None, ColorMode::Truecolor);
    assert!(text.contains('^'), "the gap marker is in the grid: {text}");
    assert!(text.contains("\x1b["), "and it is painted: {text}");
}

#[test]
fn a_day_card_lists_the_forensic_artifacts_of_that_day() {
    use timeglyph::cal::build_day;
    use timeglyph::cal_render::render_day_text;
    // The Unix epoch day carries the `unix` format epoch as an artifact.
    let d = build_day(date(1970, 1, 1), &RenderZone::Utc).unwrap();
    assert!(!d.artifacts.is_empty(), "1970-01-01 has epoch artifacts");
    let text = render_day_text(&d);
    assert!(text.contains("epoch unix @ "), "{text}");
}

// ── carve: the SYSTEMTIME lane's on-disk width ─────────────────────────────

#[test]
fn a_systemtime_carve_hit_reports_its_16_byte_region() {
    use timeglyph::carve::{carve, to_jsonl};
    // A Win32 SYSTEMTIME: eight LE u16 fields — 2020-01-01 12:00:00.000,
    // wDayOfWeek = 3 (Wednesday).
    let st: Vec<u8> = [2020u16, 1, 3, 1, 12, 0, 0, 0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let hits = carve(&st, 0.0, None);
    assert!(
        hits.iter().any(|h| h.lane.contains("SYSTEMTIME")),
        "the SYSTEMTIME lane fires: {:?}",
        hits.iter().map(|h| &h.lane).collect::<Vec<_>>()
    );
    let jsonl = to_jsonl(&hits);
    assert!(
        jsonl.lines().any(|l| l.contains("\"size\":16")),
        "the SYSTEMTIME region is 16 bytes: {jsonl}"
    );
}

#[test]
fn a_year_outside_the_civil_range_has_no_season_boundaries() {
    // `season_markers` takes an i16, so a caller can ask for a year the solver /
    // civil range cannot place. Each unlocatable boundary is omitted rather than
    // fabricated, so the whole set comes back empty.
    #[cfg(feature = "lunisolar")]
    for year in [i16::MIN, i16::MAX] {
        assert!(
            timeglyph::cal::season_markers(year).is_empty(),
            "year {year}"
        );
    }
}

#[test]
fn a_month_built_in_a_fixed_offset_labels_the_offset() {
    use timeglyph::cal::{build_month, WeekStart};
    let m = build_month(
        2020,
        6,
        &RenderZone::parse("+05:30").unwrap(),
        WeekStart::Monday,
    )
    .unwrap();
    assert_eq!(m.zone_label, "+05:30");
    let named = build_month(
        2020,
        6,
        &RenderZone::parse("Asia/Tokyo").unwrap(),
        WeekStart::Monday,
    )
    .unwrap();
    assert_eq!(named.zone_label, "Asia/Tokyo");
}

#[test]
fn a_udf_timestamp_west_of_utc_sign_extends_its_12_bit_offset() {
    // ECMA-167 TypeAndTimezone: low 12 bits are signed minutes east of UTC.
    // 0xF88 = 3976; 3976 − 0x1000 = −120 → UTC−02:00.
    let mut b = [0u8; 12];
    b[0] = 0x88;
    b[1] = 0x0F;
    b[2] = 0xE4; // year 2020, LE
    b[3] = 0x07;
    b[4] = 1; // month
    b[5] = 1; // day
    b[6] = 12; // hour
    let west = timeglyph::compose::udf(b).unwrap();
    // Wall clock 12:00 at UTC−02:00 is 14:00Z.
    assert_eq!(
        west.to_rfc3339().as_deref(),
        Some("2020-01-01T14:00:00Z"),
        "a negative offset is subtracted, not added"
    );
    // The same fields with "no timezone" (0x800) stay as read.
    b[0] = 0x00;
    b[1] = 0x08;
    assert_eq!(
        timeglyph::compose::udf(b).unwrap().to_rfc3339().as_deref(),
        Some("2020-01-01T12:00:00Z")
    );
}

// ── cal_render: the renderer's absent-overlay and empty-month paths ─────────

#[test]
fn a_month_grid_paints_the_dst_fold_marker() {
    use timeglyph::cal::{build_month, WeekStart};
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::render_month_text;
    let ny = RenderZone::parse("America/New_York").unwrap();
    // 2020-11-01 is the US fall-back (fold).
    let m = build_month(2020, 11, &ny, WeekStart::Sunday).unwrap();
    let text = render_month_text(&m, None, ColorMode::Truecolor);
    assert!(text.contains('v'), "the fold marker is in the grid: {text}");
}

#[test]
fn a_month_with_no_days_renders_the_grid_without_an_overlay_footer() {
    use timeglyph::cal::CalMonth;
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::render_month_text;
    let empty = CalMonth {
        year: 2020,
        month: 6,
        zone_label: "UTC".to_string(),
        weeks: Vec::new(),
        days: Vec::new(),
    };
    let text = render_month_text(&empty, None, ColorMode::Mono);
    assert!(text.contains("* today"), "the legend still renders: {text}");
    assert!(
        !text.contains("moon around mid-month"),
        "no representative day → no overlay footer: {text}"
    );
}

#[cfg(all(feature = "lunisolar", feature = "altcal"))]
#[test]
fn the_month_footer_skips_days_that_carry_no_alternative_calendars() {
    use timeglyph::cal::{build_day, CalMonth};
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::render_month_text;
    let first = build_day(date(2020, 6, 1), &RenderZone::Utc).unwrap();
    let second = build_day(date(2020, 6, 2), &RenderZone::Utc).unwrap();
    // A day with no alternative calendars at all (as a lean/partial build would
    // produce) must be stepped over, not treated as a month boundary.
    let mut bare = first.clone();
    bare.extra_calendars.clear();
    let m = CalMonth {
        year: 2020,
        month: 6,
        zone_label: "UTC".to_string(),
        weeks: vec![vec![Some(0), Some(1)]],
        days: vec![bare, second],
    };
    let text = render_month_text(&m, None, ColorMode::Mono);
    assert!(
        text.contains("Republic of China"),
        "the footer still names the calendars found: {text}"
    );
}

#[cfg(feature = "lunisolar")]
#[test]
fn a_day_card_without_moon_or_season_omits_those_overlays() {
    use timeglyph::cal::build_day;
    use timeglyph::cal_color::ColorMode;
    use timeglyph::cal_render::render_day_text_with;
    let mut d = build_day(date(2020, 6, 15), &RenderZone::Utc).unwrap();
    assert!(
        d.moon.is_some() && d.season.is_some(),
        "the built day has both"
    );
    d.moon = None;
    d.season = None;
    d.solar_longitude_deg = None;
    let text = render_day_text_with(&d, ColorMode::Mono);
    assert!(text.contains("2020-06-15"), "{text}");
    assert!(!text.contains("illuminated"), "no moon block: {text}");
    assert!(!text.contains("hemisphere"), "no season row: {text}");
}

// ── csv: a cell no decoder accepts under a packed format ────────────────────

#[cfg(feature = "csv")]
#[test]
fn a_packed_format_that_rejects_a_cell_both_ways_renders_empty() {
    use timeglyph::csv_enrich::{enrich, Conversion, EnrichOptions};
    // DTTM packs month/day/hour/minute: the value 0 has month 0 and day 0, which
    // no civil date accepts, so neither the integer nor the float decoder yields
    // an instant and the enriched cell is left empty.
    let out = enrich(
        "ts\n0\n",
        &EnrichOptions {
            conversions: vec![Conversion {
                column: "ts".to_string(),
                format: "dttm".to_string(),
            }],
            auto: false,
            replace: false,
            zone: RenderZone::Utc,
        },
    )
    .unwrap();
    assert_eq!(out, "ts,ts_dttm\n0,\n", "{out}");
}
