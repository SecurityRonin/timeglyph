//! Catalog build-out anchors, each cross-checked against the MIT
//! `time-decode` oracle: every (value → expected) pair below is the oracle's own
//! published example value AND its decoded answer (tier-1; see tests/oracle.rs
//! for the live differential battery). Anchors also pin value-0 = the format's
//! documented epoch.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::{format, interpret};

fn assert_int(id: &str, value: i64, prefix: &str) {
    let f = format(id).unwrap();
    let inst = f.decode_int(value).unwrap();
    let rendered = inst.to_rfc3339().unwrap_or_default();
    assert!(
        rendered.starts_with(prefix),
        "{id}({value}) = {rendered:?}, expected to start {prefix:?}"
    );
}

fn assert_float(id: &str, value: f64, prefix: &str) {
    let f = format(id).unwrap();
    let inst = f.decode_float(value).unwrap();
    let rendered = inst.to_rfc3339().unwrap_or_default();
    assert!(
        rendered.starts_with(prefix),
        "{id}({value}) = {rendered:?}, expected to start {prefix:?}"
    );
}

/// A string/hex form must surface a candidate with the given id and rendering.
fn assert_form(cands: &[interpret::Candidate], id: &str, prefix: &str) {
    let c = cands
        .iter()
        .find(|c| c.format_id == id)
        .unwrap_or_else(|| panic!("no {id} candidate among {cands:?}"));
    let r = c.rendered.as_deref().unwrap_or("");
    assert!(r.starts_with(prefix), "{id} = {r:?}, expected {prefix:?}");
}

// --- Linear formats (epoch + unit), oracle example value → oracle answer ------

#[test]
fn active_directory_filetime() {
    assert_int("active", 0, "1601-01-01T00:00:00");
    assert_int("active", 133_908_455_300_649_390, "2025-05-04T15:18:50");
}

#[test]
fn mozilla_prtime_micros_since_1970() {
    assert_int("prtime", 0, "1970-01-01T00:00:00");
    assert_int("prtime", 1_746_371_930_064_939, "2025-05-04T15:18:50");
}

#[test]
fn ios_nsdate_nanos_since_2001() {
    assert_int("iostime", 0, "2001-01-01T00:00:00");
    assert_int("iostime", 768_064_730_064_939_008, "2025-05-04T15:18:50");
}

#[test]
fn ksuid_seconds_since_2014() {
    // KSUID epoch = Unix 1_400_000_000 = 2014-05-13T16:53:20Z.
    assert_int("ksuid", 0, "2014-05-13T16:53:20");
    assert_int("ksuid", 346_371_930, "2025-05-04T15:18:50");
}

#[test]
fn excel_1904_float_days() {
    assert_float("excel1904", 0.0, "1904-01-01T00:00:00");
    assert_float("excel1904", 44_319.638_079_455_3, "2025-05-04T15:18:50");
}

// --- Embedded-ID formats (general unit, not only milliseconds) ----------------

#[test]
fn mastodon_embedded_millis_shift16() {
    assert_int("mastodon", 0, "1970-01-01T00:00:00");
    assert_int("mastodon", 114_450_230_804_480_000, "2025-05-04T15:18:50");
}

#[test]
fn linkedin_embedded_millis_shift22() {
    assert_int("linkedin", 7_324_176_984_442_343_424, "2025-05-02T21:04:29");
}

#[test]
fn tiktok_embedded_seconds_shift32() {
    // The generalised embedded strategy: TikTok's time field is SECONDS, shift 32
    // (not milliseconds). (1<<32) therefore decodes to exactly Unix second 1.
    assert_int("tiktok", 1 << 32, "1970-01-01T00:00:01");
    assert_int("tiktok", 7_228_142_017_547_750_661, "2023-05-01T09:22:38");
}

// --- String forms -------------------------------------------------------------

#[test]
fn ulid_string_first_48_bits_are_unix_millis() {
    let cands = interpret::interpret_string("01JTDY1SYGCZWCBPCSEBHV1DW2");
    assert_form(&cands, "ulid", "2025-05-04T15:18:50.064");
}

#[test]
fn uuid_v1_100ns_since_1582() {
    let cands = interpret::interpret_string("d93026f0-e857-11ed-a05b-0242ac120003");
    assert_form(&cands, "uuid_v1", "2023-05-01T19:39:12");
}

#[test]
fn rfc2822_email_date() {
    let cands = interpret::interpret_string("Sun, 04 May 2025 15:18:50 +0000");
    assert_form(&cands, "rfc2822", "2025-05-04T15:18:50");
}

#[test]
fn exif_datetime_colon_separated() {
    // EXIF DateTimeOriginal: "YYYY:MM:DD HH:MM:SS", no offset → assumed UTC.
    let cands = interpret::interpret_string("2025:05:04 15:18:50");
    assert_form(&cands, "exif", "2025-05-04T15:18:50");
}

// --- Packed: 128-bit SYSTEMTIME (hex) -----------------------------------------

#[test]
fn systemtime_128bit_struct() {
    // 8× little-endian u16: year, month, dow, day, hour, minute, second, millis.
    let groups = interpret::interpret_hex("e9070500000004000f00120032004000").unwrap();
    let all: Vec<_> = groups.iter().flat_map(|(_, c)| c.clone()).collect();
    assert_form(&all, "systemtime", "2025-05-04T15:18:50");
}

#[test]
fn google_ei_url_parameter_first_4_bytes_le_unix_seconds() {
    // Google's `ei=` URL parameter is urlsafe base64; its leading 4 bytes are a
    // little-endian Unix-seconds count. Recognised only via the `ei=` marker, so
    // a bare base64-looking token is not blindly decoded (the format IS the URL
    // parameter). Value + answer authored by the time-decode oracle (--eitime).
    let cands = interpret::interpret_string("ei=Yx1sYw");
    assert_form(&cands, "google_ei", "2022-11-09T21:36:35");
    // Also works embedded in a full query string, stopping at the `&`.
    let url = interpret::interpret_string("https://www.google.com/search?q=x&ei=Yx1sYw&oq=x");
    assert_form(&url, "google_ei", "2022-11-09T21:36:35");
    // No `ei=` marker → not decoded (noise gate): a bare token yields nothing.
    assert!(
        !interpret::interpret_string("Yx1sYw")
            .iter()
            .any(|c| c.format_id == "google_ei"),
        "a bare base64 token must not be blindly read as a Google ei timestamp"
    );
}

#[test]
fn apache_clf_datetime() {
    // Apache/nginx common-log-format date, with and without the surrounding
    // brackets. -0700 offset normalised to UTC. Oracle: CPython strptime.
    assert_form(
        &interpret::interpret_string("10/Oct/2000:13:55:36 -0700"),
        "clf",
        "2000-10-10T20:55:36",
    );
    assert_form(
        &interpret::interpret_string("[10/Oct/2000:13:55:36 -0700]"),
        "clf",
        "2000-10-10T20:55:36",
    );
}

#[test]
fn pdf_date_string() {
    // PDF metadata date: D:YYYYMMDDHHmmSS±HH'mm'. +08'00' → UTC.
    assert_form(
        &interpret::interpret_string("D:20260709123456+08'00'"),
        "pdf_date",
        "2026-07-09T04:34:56",
    );
}

#[test]
fn dmtf_cim_datetime() {
    // DMTF/WMI CIM_DATETIME: yyyymmddHHMMSS.mmmmmm±UUU (UUU = minutes east of UTC).
    // +480 = UTC+8. A staple of WMI-persistence IR (__EventFilter, SCCM).
    assert_form(
        &interpret::interpret_string("20260709123456.123456+480"),
        "dmtf_cim",
        "2026-07-09T04:34:56",
    );
}

#[test]
fn f64_bit_reinterpret_lane_decodes_a_double_timestamp() {
    // 8 bytes reinterpreted as an IEEE-754 double: 0xec4f086ddee3c641 (LE) is the
    // double 768064730.064939 = cocoa_float 2025-05-04. A raw double is how Apple
    // Biome/SEGB, bplist, and CFAbsoluteTime blobs store time — the hex path
    // decoded only integers before this lane.
    let le = interpret::interpret_hex("ec4f086ddee3c641").unwrap();
    let all_le: Vec<_> = le.iter().flat_map(|(_, c)| c.clone()).collect();
    assert_form(&all_le, "cocoa_float", "2025-05-04T15:18:50");
    // Big-endian bytes recover it via the BE f64 lane.
    let be = interpret::interpret_hex("41c6e3de6d084fec").unwrap();
    let all_be: Vec<_> = be.iter().flat_map(|(_, c)| c.clone()).collect();
    assert_form(&all_be, "cocoa_float", "2025-05-04T15:18:50");
}
