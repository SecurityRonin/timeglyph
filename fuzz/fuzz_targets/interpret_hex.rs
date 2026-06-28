#![no_main]
//! Invariant: no byte sequence may panic the hex byte-decode + interpret path.
//! Malformed hex must return an `Err`, never crash.
use libfuzzer_sys::fuzz_target;
use timeglyph::interpret;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = interpret::interpret_hex(s);
    }
});
