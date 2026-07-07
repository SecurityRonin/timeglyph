//! Rust-crate differential oracle: independent crate implementations
//! cross-check timeglyph's decoders, raising them from self-validated to
//! crate-oracle-validated (tier-2 evidence: real crate output, independently
//! authored, with ground truth derivable from the documented spec).
//!
//! Three cross-checks:
//! 1. `nt-time` (Apache-2.0 OR MIT) — FILETIME integer → Unix nanoseconds.
//! 2. `ulid` (MIT) — ULID string → Unix milliseconds.
//! 3. `uuid` (Apache-2.0 OR MIT) — UUIDv1 string → Unix seconds.
//!
//! All use fixed, documented known values so the expected result is derivable
//! from first principles independently of either implementation.
#![allow(clippy::unwrap_used, clippy::expect_used)]

// ── helpers ──────────────────────────────────────────────────────────────────

/// timeglyph's FILETIME decode, returned as Unix nanoseconds (i128).
fn tg_filetime_ns(value: i64) -> i128 {
    timeglyph::format("filetime")
        .unwrap()
        .decode_int(value)
        .unwrap()
        .0
}

/// timeglyph's interpret_string result for the given string and format_id,
/// returned as Unix nanoseconds (i128). Panics if the format_id is not found.
fn tg_string_ns(s: &str, format_id: &str) -> i128 {
    timeglyph::interpret::interpret_string(s)
        .into_iter()
        .find(|c| c.format_id == format_id)
        .unwrap_or_else(|| panic!("timeglyph did not produce a {format_id:?} candidate for {s:?}"))
        .instant
        .0
}

// ── 1. FILETIME — nt-time oracle ─────────────────────────────────────────────

/// Cross-check: timeglyph filetime decode vs nt_time::FileTime for a known
/// FILETIME value.
///
/// Known value: 132_223_104_000_000_000
///   = 2020-01-01T00:00:00Z (the lib's own doc-example, cross-checked here
///     against an independent crate).
///
/// nt-time API (v0.15): `FileTime::new(u64)` + `ft.to_unix_time_nanos() -> i128`.
/// The crate accepts a `u64` raw FILETIME; we cast from `i64` because FILETIME
/// values used in forensics always fit in a u64 (the high bit is never set for
/// any date in the supported range).
#[test]
fn nt_time_agrees_on_filetime_2020_01_01() {
    // 2020-01-01T00:00:00Z expressed as a Windows FILETIME (100ns since 1601-01-01).
    // Independently derivable: (2020-01-01 epoch offset) = 11_644_473_600 s to Unix
    // + 18_262 days × 86_400 s/day from 1970 to 2020 = 1_577_836_800 s.
    // FILETIME = (1_577_836_800 + 11_644_473_600) × 10_000_000 = 132_223_104_000_000_000.
    const FT: i64 = 132_223_104_000_000_000;

    let tg_posix_ns = tg_filetime_ns(FT);

    // nt-time: construct FileTime and convert to Unix nanoseconds.
    let ft = nt_time::FileTime::new(FT as u64);
    let oracle_ns: i128 = ft.to_unix_time_nanos();

    assert_eq!(
        tg_posix_ns, oracle_ns,
        "timeglyph filetime ns ({tg_posix_ns}) vs nt-time FileTime ns ({oracle_ns})"
    );

    // Belt-and-suspenders: both should land at exactly 2020-01-01T00:00:00Z.
    assert_eq!(
        tg_posix_ns, 1_577_836_800_000_000_000_i128,
        "expected 2020-01-01T00:00:00Z in nanoseconds"
    );
}

/// A second FILETIME anchor: the Unix epoch itself (FILETIME = 116_444_736_000_000_000).
/// nt-time documents this as `FileTime::UNIX_EPOCH`; both sides must return 0 ns.
#[test]
fn nt_time_agrees_on_filetime_unix_epoch() {
    // FILETIME for 1970-01-01T00:00:00Z: 11_644_473_600 s × 10_000_000 ticks/s.
    const FT: i64 = 116_444_736_000_000_000;

    let tg_posix_ns = tg_filetime_ns(FT);
    let oracle_ns: i128 = nt_time::FileTime::new(FT as u64).to_unix_time_nanos();

    assert_eq!(
        tg_posix_ns, oracle_ns,
        "Unix-epoch FILETIME: timeglyph ns vs nt-time ns"
    );
    assert_eq!(tg_posix_ns, 0, "FILETIME Unix-epoch must decode to 0 ns");
}

// ── 2. ULID — ulid crate oracle ───────────────────────────────────────────────

/// Cross-check: timeglyph interpret_string(ulid) → Unix ms vs ulid::Ulid::timestamp_ms().
///
/// Known value: "01ARZ3NDEKTSV4RRFFQ69G5FAV"
///   Timestamp bits encode Unix ms = 1_469_918_176_385.
///   Source: the ULID specification's reference example
///   (https://github.com/ulid/spec, README §Example).
///
/// ulid API (v1.2): `Ulid::from_string(&str) -> Result<Ulid, _>`
///                  `ulid.timestamp_ms() -> u64`
#[test]
fn ulid_crate_agrees_on_spec_example() {
    // The ULID spec's canonical example string.
    const ULID_STR: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    let tg_posix_ns = tg_string_ns(ULID_STR, "ulid");
    // timeglyph stores PosixNs; convert to milliseconds for comparison.
    let tg_unix_ms = tg_posix_ns / 1_000_000;

    // ulid crate: parse and extract milliseconds.
    let oracle_ms = i128::from(
        ulid::Ulid::from_string(ULID_STR)
            .expect("ulid crate must parse the spec example string")
            .timestamp_ms(),
    );

    assert_eq!(
        tg_unix_ms, oracle_ms,
        "timeglyph ULID ms ({tg_unix_ms}) vs ulid crate ms ({oracle_ms})"
    );
}

/// A second ULID anchor: a ULID whose timestamp encodes a clean second boundary
/// (ms = 1_000_000_000_000, i.e. 2001-09-09T01:46:40Z).
/// Constructed from known parts: timestamp_ms=1_000_000_000_000, random=0.
#[test]
fn ulid_crate_agrees_on_round_second() {
    // Build the ULID from known parts so both sides have the same source of truth.
    // timestamp_ms = 1_000_000_000_000 (2001-09-09T01:46:40.000Z)
    const TS_MS: u64 = 1_000_000_000_000;
    let constructed = ulid::Ulid::from_parts(TS_MS, 0);
    let ulid_str = constructed.to_string();

    let tg_posix_ns = tg_string_ns(&ulid_str, "ulid");
    let tg_unix_ms = tg_posix_ns / 1_000_000;

    let oracle_ms = i128::from(constructed.timestamp_ms());

    assert_eq!(
        tg_unix_ms, oracle_ms,
        "timeglyph ULID ms ({tg_unix_ms}) vs ulid crate ms ({oracle_ms}) for {ulid_str}"
    );
    // Both should land at 2001-09-09T01:46:40Z = Unix second 1_000_000_000.
    assert_eq!(
        tg_unix_ms,
        i128::from(TS_MS),
        "round-second ULID timestamp_ms mismatch"
    );
}

// ── 3. UUIDv1 — uuid crate oracle ────────────────────────────────────────────

/// Cross-check: timeglyph interpret_string(uuid_v1) → Unix seconds vs
/// uuid::Uuid::get_timestamp() → Timestamp::to_unix().
///
/// Known value: "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
///   This is the DNS namespace UUID, defined in RFC 4122 Appendix C / RFC 9562
///   Appendix C. Its version nibble is 1 (time-based).
///   The Gregorian timestamp encodes 1998-10-22 at approximately 02:25:42 UTC.
///   (RFC 4122 §C defines this UUID; the timestamp is verifiable from its hex.)
///
/// uuid API (v1.23): `Uuid::parse_str(&str) -> Result<Uuid, _>`
///                   `uuid.get_timestamp() -> Option<Timestamp>`
///                   `ts.to_unix() -> (u64 secs, u32 subsec_nanos)`
#[test]
fn uuid_crate_agrees_on_rfc4122_dns_namespace_uuid() {
    // RFC 4122 Appendix C / RFC 9562 Appendix C: the well-known DNS namespace UUID.
    // Version nibble = 1 (time-based), so get_timestamp() must return Some(_).
    const UUID_STR: &str = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";

    let tg_posix_ns = tg_string_ns(UUID_STR, "uuid_v1");
    let tg_unix_secs = tg_posix_ns / 1_000_000_000;

    // uuid crate: parse, extract timestamp, convert to Unix seconds.
    let u = uuid::Uuid::parse_str(UUID_STR).expect("uuid crate must parse the DNS UUID");
    let ts = u
        .get_timestamp()
        .expect("DNS namespace UUID is version 1 — get_timestamp must return Some");
    let (oracle_secs_u64, _subsec_nanos) = ts.to_unix();
    let oracle_secs = i128::from(oracle_secs_u64);

    assert_eq!(
        tg_unix_secs, oracle_secs,
        "timeglyph UUIDv1 secs ({tg_unix_secs}) vs uuid crate secs ({oracle_secs})"
    );
}

/// A second UUIDv1 anchor: a UUID whose Gregorian timestamp is constructed so
/// both sides agree on a known Unix second — 2020-01-01T00:00:00Z.
///
/// UUIDv1 Gregorian ticks for 2020-01-01T00:00:00Z:
///   Unix seconds to Gregorian: 1_577_836_800 + 12_219_292_800 = 13_797_129_600 s
///   × 10_000_000 ticks/s = 137_971_296_000_000_000 (60-bit Gregorian ticks)
///
/// We build the UUID hex manually from those ticks to avoid needing the uuid
/// crate's `v1` constructor (which requires a ClockSequence context), keeping
/// the test self-contained.
#[test]
fn uuid_crate_agrees_on_2020_epoch() {
    // 2020-01-01T00:00:00Z as Gregorian 100ns ticks (UUID epoch 1582-10-15):
    //   Unix second 1_577_836_800 + 12_219_292_800 s (Gregorian-to-Unix offset)
    //   = 13_797_129_600 s × 10_000_000 = 137_971_296_000_000_000 ticks.
    const GREGORIAN_TICKS: u64 = 137_971_296_000_000_000;

    // Pack into UUIDv1 layout (RFC 9562 §5.1):
    //   time_low  = ticks[31:0]
    //   time_mid  = ticks[47:32]
    //   time_hi_version = 0x1000 | ticks[59:48]
    let time_low = (GREGORIAN_TICKS & 0xFFFF_FFFF) as u32;
    let time_mid = ((GREGORIAN_TICKS >> 32) & 0xFFFF) as u16;
    let time_hi_ver = (0x1000u16) | (((GREGORIAN_TICKS >> 48) & 0x0FFF) as u16);
    // clock_seq and node are arbitrary non-zero values (not time-bearing).
    let clock_seq_hi_res: u8 = 0x80; // variant bits 10xxxxxx
    let clock_seq_low: u8 = 0x00;
    let node: [u8; 6] = [0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8];

    let uuid_str = format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        time_low,
        time_mid,
        time_hi_ver,
        clock_seq_hi_res,
        clock_seq_low,
        node[0],
        node[1],
        node[2],
        node[3],
        node[4],
        node[5],
    );

    let tg_posix_ns = tg_string_ns(&uuid_str, "uuid_v1");
    let tg_unix_secs = tg_posix_ns / 1_000_000_000;

    let u = uuid::Uuid::parse_str(&uuid_str).expect("uuid crate must parse the constructed UUID");
    let ts = u
        .get_timestamp()
        .expect("constructed UUIDv1 must have a timestamp");
    let (oracle_secs_u64, _subsec_nanos) = ts.to_unix();
    let oracle_secs = i128::from(oracle_secs_u64);

    assert_eq!(
        tg_unix_secs, oracle_secs,
        "timeglyph UUIDv1 secs ({tg_unix_secs}) vs uuid crate secs ({oracle_secs}) for {uuid_str}"
    );
    assert_eq!(
        tg_unix_secs, 1_577_836_800_i128,
        "2020-01-01T00:00:00Z UUIDv1 must decode to Unix second 1_577_836_800"
    );
}
