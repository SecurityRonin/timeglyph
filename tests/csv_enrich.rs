//! CSV enrichment: add human-readable timestamp columns to a CSV, either by
//! auto-detecting numeric timestamp columns or by explicit column→format
//! conversion, optionally replacing the original column.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::csv_enrich::{enrich, Conversion, EnrichOptions};

fn header(csv: &str) -> String {
    csv.lines().next().unwrap_or_default().to_string()
}

#[test]
fn explicit_conversion_adds_column_to_the_right() {
    let input = "id,created\n1,1577836800\n2,1583020800\n";
    let opts = EnrichOptions {
        conversions: vec![Conversion {
            column: "created".into(),
            format: "unix".into(),
        }],
        auto: false,
        replace: false,
    };
    let out = enrich(input, &opts).unwrap();
    // New column appears immediately to the right of the source column.
    assert!(
        header(&out).contains("created,created_unix"),
        "header: {}",
        header(&out)
    );
    // Original value preserved, formatted value added.
    assert!(out.contains("1577836800"), "{out}");
    assert!(out.contains("2020-01-01T00:00:00Z"), "{out}");
    assert!(out.contains("2020-03-01T00:00:00Z"), "{out}");
}

#[test]
fn replace_option_replaces_the_original_column() {
    let input = "id,created\n1,1577836800\n";
    let opts = EnrichOptions {
        conversions: vec![Conversion {
            column: "created".into(),
            format: "unix".into(),
        }],
        auto: false,
        replace: true,
    };
    let out = enrich(input, &opts).unwrap();
    assert_eq!(header(&out), "id,created", "header unchanged on replace");
    assert!(out.contains("2020-01-01T00:00:00Z"), "{out}");
    assert!(
        !out.contains("1577836800"),
        "original value must be replaced"
    );
}

#[test]
fn auto_detects_a_unix_seconds_column() {
    let input = "name,ts\na,1577836800\nb,1583020800\n";
    let opts = EnrichOptions {
        conversions: vec![],
        auto: true,
        replace: false,
    };
    let out = enrich(input, &opts).unwrap();
    assert!(header(&out).contains("ts_unix"), "header: {}", header(&out));
    assert!(out.contains("2020-01-01T00:00:00Z"), "{out}");
}

#[test]
fn auto_skips_small_numeric_columns() {
    // 5 and 42 are far too small to be plausible timestamps — must NOT be
    // enriched (a count column, not a time column).
    let input = "id,count\n1,5\n2,42\n";
    let opts = EnrichOptions {
        conversions: vec![],
        auto: true,
        replace: false,
    };
    let out = enrich(input, &opts).unwrap();
    assert_eq!(
        header(&out),
        "id,count",
        "small numeric column must be skipped"
    );
}
