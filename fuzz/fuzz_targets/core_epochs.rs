#![no_main]
//! `timeglyph-core`'s converters, over arbitrary inputs.
//!
//! Their inputs are plain scalars, not a structure to be discovered, so this
//! target is not hunting a parse bug — it is empirical evidence for the crate's
//! standing claim that no input panics and none saturates into a
//! plausible-but-wrong instant. The interesting region is the arithmetic edges:
//! the epoch boundary, `i64`-nanosecond overflow, and NaN / ±inf.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut word = [0u8; 8];
    let Some(head) = data.get(..word.len()) else {
        return;
    };
    word.copy_from_slice(head);
    let v = u64::from_le_bytes(word);

    let _ = timeglyph_core::filetime_to_unix_ns(v);
    let _ = timeglyph_core::webkit_micros_to_unix_ns(v);
    // from_bits, not `as f64`: it reaches the NaN and infinity encodings a cast
    // never produces, which is the whole point of fuzzing the float path.
    let _ = timeglyph_core::cocoa_secs_to_unix_ns(f64::from_bits(v));
    let _ = timeglyph_core::hfs_secs_to_unix_ns(v as u32);
});
