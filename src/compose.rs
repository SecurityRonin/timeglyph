//! Composite (two-word) timestamp decode: values split across two integer
//! fields. Some artifacts store a timestamp as two halves rather than one
//! integer — a FILETIME as its `dwLowDateTime`/`dwHighDateTime` DWORDs in `.reg`
//! exports, IE `index.dat` cookies, and packed malware configs. This reassembles
//! the halves and decodes via the canonical single-value path, so the same
//! epoch math applies. No single-value converter reconstructs these.

use crate::{ChronoError, PosixNs};

/// Reconstruct a Windows FILETIME from its low and high 32-bit halves and decode
/// it as 100 ns since 1601. `FILETIME = (high << 32) | low` — the order the two
/// DWORDs carry in a `FILETIME`/`Windows Cookie` structure.
///
/// # Errors
/// Returns [`ChronoError`] if the reconstructed value is out of the decodable
/// range (never panics).
pub fn filetime_hilo(low: u32, high: u32) -> Result<PosixNs, ChronoError> {
    let ft = (u64::from(high) << 32) | u64::from(low);
    let ticks = i64::try_from(ft).map_err(|_| ChronoError::OutOfRange {
        what: "filetime",
        value: i128::from(ft),
    })?;
    crate::format("filetime")?.decode_int(ticks)
}
