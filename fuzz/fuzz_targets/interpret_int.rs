#![no_main]
//! Invariant: no `i64` value may panic the integer auto-detect path, and every
//! surfaced candidate must render without panicking.
use libfuzzer_sys::fuzz_target;
use timeglyph::interpret;

fuzz_target!(|value: i64| {
    let cands = interpret::interpret_int(value);
    for c in &cands {
        let _ = c.rendered.as_deref();
        let _ = c.score;
    }
});
