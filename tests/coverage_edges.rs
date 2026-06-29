//! Error-path edges: the reachable failure arms the happy-path suites miss.
//! Provably-dead arms (in-memory Vec writes, UTF-8 of our own strings, to_zoned
//! of an already-valid datetime) are instead annotated `// cov:unreachable`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::csv_enrich::{enrich, Conversion, EnrichOptions};
use timeglyph::{format, interpret, PosixNs, RenderZone};

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
