//! WASM bindings for the timeglyph engine — the browser playground's entry
//! point. A thin wrapper over [`timeglyph::interpret::identify_json`]; the engine
//! itself is `forbid(unsafe_code)`, and this crate isolates wasm-bindgen's
//! generated `unsafe` glue so that guarantee is unaffected.

use wasm_bindgen::prelude::wasm_bindgen;

/// Identify a timestamp value across every format family, returning the ranked
/// readings (with citations) as a JSON string. Undecodable input yields `"[]"`.
#[wasm_bindgen]
#[must_use]
pub fn identify(value: &str) -> String {
    timeglyph::interpret::identify_json(value)
}
