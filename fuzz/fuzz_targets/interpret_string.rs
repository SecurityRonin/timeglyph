#![no_main]
//! Invariant: no string may panic the string-form parse path (ISO 8601 / RFC
//! 3339 / RFC 2822 / ASN.1 UTCTime+GeneralizedTime / ULID / UUIDv1 / EXIF).
//! Unparseable input must yield an empty candidate list, never crash.
use libfuzzer_sys::fuzz_target;
use timeglyph::interpret;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let cands = interpret::interpret_string(s);
        for c in &cands {
            let _ = c.rendered.as_deref();
            let _ = c.score;
        }
    }
});
