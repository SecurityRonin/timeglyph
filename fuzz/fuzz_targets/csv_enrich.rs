#![no_main]
//! Invariant: no CSV input may panic the auto-detect enrichment path. Malformed
//! CSV must return an `Err` or a best-effort string, never crash.
use libfuzzer_sys::fuzz_target;
use timeglyph::csv_enrich::{enrich, EnrichOptions};
use timeglyph::RenderZone;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let opts = EnrichOptions {
            conversions: Vec::new(),
            auto: true,
            replace: false,
            zone: RenderZone::Utc,
        };
        let _ = enrich(s, &opts);
    }
});
