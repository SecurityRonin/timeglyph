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

/// The `google_ei` reading of `input`: its rendered instant and assumption text.
fn ei_reading(input: &str) -> (String, String) {
    let c = interpret::interpret_string(input)
        .into_iter()
        .find(|c| c.format_id == "google_ei")
        .unwrap_or_else(|| panic!("no google_ei candidate for {input:?}"));
    (c.rendered.unwrap_or_default(), c.assumptions.join(" "))
}

#[test]
fn google_ei_decodes_the_microsecond_varint_after_the_seconds() {
    // Real URL from unfurl issue #56 (Rasmus-Riis, 2020). After the 4-byte LE
    // seconds comes a protobuf varint of microseconds (540099). Corroborated by a
    // separate Google field in the SAME URL: `ved` protobuf 13→1→1 carries
    // 1587403446540099 µs, and unfurl renders the same µs value.
    let (r, note) =
        ei_reading("https://www.google.com/search?ei=ttqdXsP7IMKZk74Pgv-k6AY&q=third+search");
    assert_eq!(r, "2020-04-20T17:24:06.540099Z");
    assert!(note.contains("microsecond"), "{note}");
    // Cheeky4n6Monkey / Deed Poll Office example (seconds 1387841717 published
    // there; the µs varint is 616780).
    let (r, _) = ei_reading("ei=tci4UszSJeLN7Ab9xYD4CQ");
    assert_eq!(r, "2013-12-23T23:35:17.616780Z");
}

#[test]
fn google_ei_says_it_is_the_page_serve_time_not_the_query_time() {
    // unfurl #56: ei was minted when Google served the page the user searched
    // FROM (session start / previous search), minutes to hours before the query
    // in the same URL. The reading must carry that caveat, not imply search time.
    let (_, note) = ei_reading("ei=ttqdXsP7IMKZk74Pgv-k6AY");
    assert!(note.contains("not necessarily"), "{note}");
}

#[test]
fn google_ei_without_a_usable_microsecond_field_says_so() {
    // Synthetic (python: urlsafe_b64encode(pack('<I',1587403446)+tail)):
    // tail 0xC3 (varint never terminates) and tail C0 84 3D (= 1_000_000, not a
    // microsecond count) — both fall back to whole seconds and SAY so, rather
    // than inventing a fraction or rendering a bare second as if it were exact.
    for tok in ["ttqdXsM", "ttqdXsCEPQ", "Yx1sYw"] {
        let (r, note) = ei_reading(&format!("ei={tok}"));
        assert!(r.ends_with(":06Z") || tok == "Yx1sYw", "{tok}: {r}");
        assert!(note.contains("whole seconds"), "{tok}: {note}");
    }
    // 999_999 is the largest valid microsecond value.
    let (r, _) = ei_reading("ei=ttqdXr-EPQ");
    assert_eq!(r, "2020-04-20T17:24:06.999999Z");
}

#[test]
fn google_ei_matches_only_the_ei_and_sei_parameter_names() {
    // `sei=` carries the same encoding (Cheeky4n6Monkey 2014: sei and ei from one
    // session share their leading bytes).
    let (r, _) =
        ei_reading("https://www.google.com.au/search?q=bananas&gbv=1&sei=BrU2VKfrB9Xz8gX2iILoBA");
    assert!(r.starts_with("2014-10-09T16:17:10"), "{r}");
    // A parameter merely ENDING in "ei" is a different parameter.
    for other in [
        "?gei=ttqdXsP7IMKZk74Pgv-k6AY",
        "x?q=1&rei=ttqdXsP7IMKZk74Pgv-k6AY",
    ] {
        assert!(
            !interpret::interpret_string(other)
                .iter()
                .any(|c| c.format_id == "google_ei"),
            "{other} is not an ei= parameter"
        );
    }
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
    let little = interpret::interpret_hex("ec4f086ddee3c641").unwrap();
    let from_little: Vec<_> = little.iter().flat_map(|(_, c)| c.clone()).collect();
    assert_form(&from_little, "cocoa_float", "2025-05-04T15:18:50");
    // Big-endian bytes recover it via the BE f64 lane.
    let big = interpret::interpret_hex("41c6e3de6d084fec").unwrap();
    let from_big: Vec<_> = big.iter().flat_map(|(_, c)| c.clone()).collect();
    assert_form(&from_big, "cocoa_float", "2025-05-04T15:18:50");
}

#[test]
fn jwt_payload_claims_decode_as_unix_seconds() {
    // A JWT (header.payload.signature, base64url): the payload's iat/exp/nbf are
    // Unix seconds. iat=1577836800 (2020-01-01). SOC/token-theft triage.
    let jwt = "eyJhbGciOiAiSFMyNTYiLCAidHlwIjogIkpXVCJ9.eyJpYXQiOiAxNTc3ODM2ODAwLCAiZXhwIjogMTU3NzkyMzIwMCwgInN1YiI6ICJ4In0.SIGNATURE";
    let cands = interpret::interpret_string(jwt);
    assert_form(&cands, "jwt_iat", "2020-01-01T00:00:00");
    assert_form(&cands, "jwt_exp", "2020-01-02T00:00:00");
}
