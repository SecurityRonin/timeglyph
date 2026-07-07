//! Error-path edges: the reachable failure arms the happy-path suites miss.
//! Provably-dead arms (in-memory Vec writes, UTF-8 of our own strings, to_zoned
//! of an already-valid datetime) are instead annotated `// cov:unreachable`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[cfg(feature = "csv")]
use timeglyph::csv_enrich::{enrich, Conversion, EnrichOptions};
use timeglyph::{format, interpret, scan, DateStyle, PosixNs, RenderZone, TzSemantics};

#[test]
fn invalid_hex_is_an_error() {
    assert!(interpret::interpret_hex("zznothex").is_err());
}

#[test]
fn unknown_format_id_is_an_error() {
    assert!(format("no_such_format").is_err());
}

#[test]
fn malformed_fixed_offset_is_an_error() {
    // `+`/`-` prefix but malformed/out-of-range → parse_offset None → UnknownZone.
    assert!(RenderZone::parse("+25:00").is_err());
    assert!(RenderZone::parse("-99").is_err());
    assert!(RenderZone::parse("+abcd").is_err());
}

#[test]
fn encode_int_overflow_is_an_error() {
    // An instant so far out that the tick count exceeds i64 → OutOfRange.
    let f = format("unix").unwrap();
    assert!(f.encode_int(PosixNs(i128::MAX)).is_err());
}

#[cfg(feature = "csv")]
#[test]
fn csv_unknown_column_is_an_error() {
    let opts = EnrichOptions {
        conversions: vec![Conversion {
            column: "does_not_exist".to_string(),
            format: "unix".to_string(),
        }],
        auto: false,
        replace: false,
        zone: RenderZone::Utc,
    };
    assert!(enrich("id,created\n1,1577836800\n", &opts).is_err());
}

#[cfg(feature = "csv")]
#[test]
fn csv_replace_keeps_unrenderable_cells() {
    // replace=true + a non-numeric cell under a numeric format → render_cell None
    // → the original cell is kept (the `unwrap_or_else(cell)` arm).
    let opts = EnrichOptions {
        conversions: vec![Conversion {
            column: "created".to_string(),
            format: "unix".to_string(),
        }],
        auto: false,
        replace: true,
        zone: RenderZone::Utc,
    };
    let out = enrich("id,created\n1,not_a_number\n", &opts).unwrap();
    assert!(out.contains("not_a_number"), "{out}");
}

#[test]
fn negative_one_is_a_flagged_sentinel() {
    // The -1 / all-ones "unset" arm of sentinel_reason.
    assert!(interpret::sentinel_reason(-1).is_some());
    assert!(interpret::sentinel_reason(0).is_some());
    assert!(interpret::sentinel_reason(12345).is_none());
}

#[test]
fn asn1_without_a_timezone_flags_assumed_utc() {
    // A GeneralizedTime with no Z/offset → the "assumed UTC, but may be local"
    // assumption arm of asn1_assumption.
    let cands = interpret::interpret_string("20200101120000");
    assert!(
        cands.iter().any(|c| c
            .assumptions
            .iter()
            .any(|a| a.contains("NO timezone designator"))),
        "expected an assumed-UTC ASN.1 assumption, got: {cands:?}"
    );
}

#[test]
fn fixed_offset_with_three_digits_is_an_error() {
    // `+HHH` (3 digits) hits the `_ => None` arm of parse_offset's length match —
    // distinct from the 1/2/4-digit forms the happy path exercises.
    assert!(RenderZone::parse("+123").is_err());
}

#[test]
fn hex_decodes_an_eight_byte_value() {
    // A full 8-byte little/big-endian sweep drives interpret_hex's per-width loop
    // to completion (the linear-integer readings path).
    let out = interpret::interpret_hex("0102030405060708").unwrap();
    assert!(!out.is_empty());
}

#[test]
fn render_in_zone_passes_offset_embedded_native_through() {
    // OffsetEmbedded keeps the native string verbatim (it carries its own offset).
    let (rendered, lossy) = scan::render_in_zone(
        TzSemantics::OffsetEmbedded,
        PosixNs(0),
        "2020-01-01T12:00:00+08:00",
        &RenderZone::Utc,
        DateStyle::Iso8601,
    );
    assert_eq!(rendered, "2020-01-01T12:00:00+08:00");
    assert!(!lossy);
}

#[test]
fn readings_for_a_non_number_is_empty() {
    // The `let Ok(value) = number.parse::<i64>() else { return Vec::new() }` arm.
    assert!(scan::readings_for_opts(
        "not_a_number",
        5,
        false,
        &RenderZone::Utc,
        DateStyle::Iso8601
    )
    .is_empty());
}

#[test]
fn bitwise_decimal_rejects_a_negative_value() {
    // The negative-value guard of decode_bitdec (a Packed-strategy format).
    assert!(format("bitdec").unwrap().decode_int(-1).is_err());
}

#[cfg(feature = "csv")]
#[test]
fn csv_renders_a_fractional_cell_via_the_float_path() {
    // A fractional value fails i64 parse but decodes as a float → render_cell's
    // f64 fallback arm (a float-serial format).
    let opts = EnrichOptions {
        conversions: vec![Conversion {
            column: "when".to_string(),
            format: "ole".to_string(),
        }],
        auto: false,
        replace: false,
        zone: RenderZone::Utc,
    };
    let out = enrich("id,when\n1,44562.5\n", &opts).unwrap();
    // A new human-readable column was appended (the float path produced a value).
    assert!(
        out.lines().next().unwrap().matches(',').count() >= 2,
        "{out}"
    );
}

#[cfg(feature = "csv")]
#[test]
fn csv_auto_detect_skips_a_non_timestamp_column() {
    // auto-detect over a column that matches no format → detect_column_format None.
    let opts = EnrichOptions {
        conversions: Vec::new(),
        auto: true,
        replace: false,
        zone: RenderZone::Utc,
    };
    let out = enrich("id,label\n1,hello\n2,world\n", &opts).unwrap();
    // No timestamp column found → the data is returned essentially unchanged.
    assert!(out.contains("hello") && out.contains("world"), "{out}");
}
