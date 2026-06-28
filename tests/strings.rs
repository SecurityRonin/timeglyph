//! String-form timestamp parsing: ISO 8601 / RFC 3339 and ASN.1
//! UTCTime / GeneralizedTime (X.680 / RFC 5280), as found in certificates, PKI,
//! and many binary formats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

#[test]
fn parses_iso8601_rfc3339() {
    let cands = interpret::interpret_string("2020-01-01T00:00:00Z");
    assert!(
        cands
            .iter()
            .any(|c| c.rendered.as_deref() == Some("2020-01-01T00:00:00Z")),
        "expected an ISO8601/RFC3339 reading: {cands:?}"
    );
}

#[test]
fn parses_rfc3339_with_offset() {
    // 2020-01-01T00:00:00+01:00 == 2019-12-31T23:00:00Z
    let cands = interpret::interpret_string("2020-01-01T00:00:00+01:00");
    assert!(cands
        .iter()
        .any(|c| c.rendered.as_deref() == Some("2019-12-31T23:00:00Z")));
}

#[test]
fn parses_asn1_generalizedtime() {
    // GeneralizedTime: 4-digit year, YYYYMMDDHHMMSSZ.
    let cands = interpret::interpret_string("20200101000000Z");
    assert!(
        cands.iter().any(|c| c.format_id == "asn1_generalizedtime"
            && c.rendered
                .as_deref()
                .unwrap()
                .starts_with("2020-01-01T00:00:00")),
        "expected an ASN.1 GeneralizedTime reading: {cands:?}"
    );
}

#[test]
fn parses_asn1_utctime_with_rfc5280_pivot() {
    // UTCTime: 2-digit year. RFC 5280 pivot: YY < 50 => 20YY, else 19YY.
    let y2020 = interpret::interpret_string("200101000000Z");
    assert!(y2020.iter().any(|c| c.format_id == "asn1_utctime"
        && c.rendered.as_deref().unwrap().starts_with("2020-01-01")));
    let y1995 = interpret::interpret_string("950101000000Z");
    assert!(y1995.iter().any(|c| c.format_id == "asn1_utctime"
        && c.rendered.as_deref().unwrap().starts_with("1995-01-01")));
}

#[test]
fn garbage_string_yields_no_candidates() {
    assert!(interpret::interpret_string("not a timestamp").is_empty());
}

#[test]
fn asn1_generalizedtime_fractional_seconds() {
    // X.680 allows a fractional second: YYYYMMDDHHMMSS.fff.
    let cands = interpret::interpret_string("20200101000000.5Z");
    let gt = cands
        .iter()
        .find(|c| c.format_id == "asn1_generalizedtime")
        .expect("generalizedtime with fraction");
    assert!(
        gt.rendered
            .as_deref()
            .unwrap()
            .starts_with("2020-01-01T00:00:00.5"),
        "{:?}",
        gt.rendered
    );
}

#[test]
fn asn1_generalizedtime_omitted_seconds() {
    // X.680 allows omitting seconds (YYYYMMDDHHMM) and minutes (YYYYMMDDHH).
    let cands = interpret::interpret_string("2020010100Z"); // YYYYMMDDHH
    assert!(
        cands.iter().any(|c| c.format_id == "asn1_generalizedtime"
            && c.rendered
                .as_deref()
                .unwrap()
                .starts_with("2020-01-01T00:00:00")),
        "{cands:?}"
    );
}

#[test]
fn asn1_fraction_without_seconds_is_rejected() {
    // A fraction is only meaningful on the smallest present unit being seconds.
    assert!(interpret::interpret_string("2020010100.5Z")
        .iter()
        .all(|c| c.format_id != "asn1_generalizedtime"));
}
