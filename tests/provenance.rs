//! Provenance anchor: the registry digest is a stable, reproducible fingerprint
//! of the format definitions that produced a reading — the core of the
//! `--provenance` envelope (court-defensibility: "which method version decoded
//! this?").
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[test]
fn registry_digest_is_stable_16_hex() {
    let d = timeglyph::registry_digest();
    assert_eq!(d.len(), 16, "digest should be 16 hex chars: {d:?}");
    assert!(d.bytes().all(|b| b.is_ascii_hexdigit()), "hex only: {d:?}");
    // Deterministic across calls — a provenance anchor must be reproducible.
    assert_eq!(d, timeglyph::registry_digest());
}
