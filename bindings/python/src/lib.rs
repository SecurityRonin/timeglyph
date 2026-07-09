#![allow(clippy::useless_conversion)] // pyo3 `#[pyfunction]` macro `?`-desugar
//! Python bindings for the timeglyph engine (a maturin/pyo3 extension module).
//! Thin wrappers over `::timeglyph::interpret`; `unsafe` is allowed only here for
//! pyo3's generated glue, so the engine itself keeps `forbid(unsafe_code)`.

use pyo3::prelude::*;

/// Identify a timestamp value across every format family, returning the ranked
/// readings as a list of dicts (`format_id`, `rendered`, `score`, `citation`,
/// `assumptions`, `components`, ...). Empty list if nothing decodes.
///
/// Decoded via Python's `json` so the engine's `i128` instant becomes a native
/// (arbitrary-precision) Python int rather than tripping a fixed-width limit.
#[pyfunction]
fn identify(py: Python<'_>, value: &str) -> PyResult<PyObject> {
    let json = ::timeglyph::interpret::identify_json(value);
    let obj = py.import_bound("json")?.call_method1("loads", (json,))?;
    Ok(obj.unbind())
}

/// Identify a value, returning the ranked readings as a JSON string.
#[pyfunction]
fn identify_json(value: &str) -> String {
    ::timeglyph::interpret::identify_json(value)
}

#[pymodule]
fn timeglyph(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(identify, m)?)?;
    m.add_function(wrap_pyfunction!(identify_json, m)?)?;
    Ok(())
}
