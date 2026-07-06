#![no_main]
//! Invariant: no accessibility/stdin text may panic the scan path. `inspect_text`
//! is the lens's real untrusted entry point — the UTF-16 `word_at` mapping, the
//! float-dot logic in `scan_numbers_min`, and the datetime/hex extractors all
//! run on arbitrary hovered or piped text. Malformed or adversarial input must
//! yield readings or nothing, never crash, with bounded output (MAX_SCAN_BYTES).
use libfuzzer_sys::fuzz_target;
use timeglyph::RenderZone;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = timeglyph::scan::inspect_text(s, 8, &RenderZone::Utc);
    }
});
