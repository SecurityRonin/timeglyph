//! Additional catalog formats (HANDOFF §5a long tail): Modified Julian Day,
//! MongoDB ObjectId, and UUID v6/v7. Anchors are derivable from each format's
//! documented construction (tier-2): MJD 40587 = 1970-01-01; ObjectId/UUID
//! embed a Unix-seconds / Unix-ms / Gregorian-100ns timestamp at a known offset.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::{format, interpret};

fn form(cands: &[interpret::Candidate], id: &str) -> String {
    cands
        .iter()
        .find(|c| c.format_id == id)
        .unwrap_or_else(|| panic!("no {id} candidate"))
        .rendered
        .clone()
        .unwrap_or_default()
}

#[test]
fn modified_julian_day() {
    // MJD day 0 = 1858-11-17; MJD 40587 = 1970-01-01, MJD 51544 = 2000-01-01.
    let f = format("mjd").unwrap();
    assert!(f
        .decode_float(40587.0)
        .unwrap()
        .to_rfc3339()
        .unwrap()
        .starts_with("1970-01-01"));
    assert!(f
        .decode_float(51544.0)
        .unwrap()
        .to_rfc3339()
        .unwrap()
        .starts_with("2000-01-01"));
}

#[test]
fn mongodb_objectid() {
    // ObjectId's first 4 bytes are Unix seconds (big-endian): 0x5E0BE100 = 2020.
    let c = interpret::interpret_string("5e0be1000000000000000000");
    assert!(form(&c, "objectid").starts_with("2020-01-01"));
}

#[test]
fn uuid_version_7() {
    // UUIDv7: the high 48 bits are Unix milliseconds. 0x016F5E66E800 = 2020.
    let c = interpret::interpret_string("016f5e66-e800-7000-8000-000000000000");
    assert!(form(&c, "uuid_v7").starts_with("2020-01-01"));
}

#[test]
fn uuid_version_6() {
    // UUIDv6: a reordered 60-bit Gregorian (100ns since 1582-10-15) timestamp.
    // This is the same instant as the v1 example d93026f0-e857-11ed.
    let c = interpret::interpret_string("1ede857d-9302-66f0-8000-000000000000");
    assert!(form(&c, "uuid_v6").starts_with("2023-05-01T19:39:12"));
}

#[test]
fn sql_server_datetime() {
    // 8 bytes: int32 days since 1900-01-01 + uint32 ticks of 1/300 s. value 0 =
    // 1900-01-01; 43829 days (high word 0xAB35) = 2020-01-01.
    let f = format("sqlserver").unwrap();
    assert!(f
        .decode_int(0)
        .unwrap()
        .to_rfc3339()
        .unwrap()
        .starts_with("1900-01-01"));
    assert!(f
        .decode_int(0xAB35_0000_0000)
        .unwrap()
        .to_rfc3339()
        .unwrap()
        .starts_with("2020-01-01"));
}

#[test]
fn iso_ordinal_and_week_dates() {
    // ISO 8601 ordinal (day-of-year) and week dates: both 2025-05-04.
    let o = interpret::interpret_string("2025-124");
    assert!(form(&o, "iso_ordinal").starts_with("2025-05-04"));
    let w = interpret::interpret_string("2025-W18-7");
    assert!(form(&w, "iso_week").starts_with("2025-05-04"));
}
