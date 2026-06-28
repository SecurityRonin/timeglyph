//! Leap-aware time scales — GPS, TAI64, NTP — deliberately kept OUT of the
//! [`PosixNs`](crate::PosixNs) spine (HANDOFF §3). GPS and TAI are genuinely
//! leap-aware: their UTC rendering applies the IERS leap-second table via the
//! `hifitime` crate (leap math is solved; we do NOT reinvent it). NTP follows
//! UTC and does NOT count leap seconds, so its conversion is additive (reused
//! via `jiff`); it is grouped here for the era-rollover handling, not for leap
//! math.
//!
//! Every reading is evidence, not a verdict: it carries its scale, a spec
//! citation, and its assumptions, and never routes through the POSIX spine.

use crate::ChronoError;
use hifitime::Epoch;

/// A reading from a leap-aware (or era-bearing) time scale, rendered to
/// leap-correct UTC. Distinct from [`PosixNs`](crate::PosixNs): these scales
/// must not be mixed with the POSIX (leap-ignoring) majority.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LeapReading {
    /// The source time scale (`"GPS"`, `"TAI64"`, `"NTP"`).
    pub scale: &'static str,
    /// Authoritative spec citation.
    pub citation: &'static str,
    /// Leap-correct UTC rendering (RFC 3339, `Z`).
    pub utc_rfc3339: String,
    /// Stated assumptions — a reading, not a determination.
    pub assumptions: Vec<String>,
}

/// SI seconds between the hifitime TAI epoch (1900-01-01) and 1970-01-01, and
/// equivalently NTP's prime-epoch (1900) offset to the Unix epoch.
const SECS_1900_TO_1970: i64 = 2_208_988_800;
/// The TAI64 external reference: label 2^62 == 1970-01-01 00:00:00 TAI
/// (D. J. Bernstein, libtai TAI64 spec).
const TAI64_BASE: u64 = 1 << 62;

/// Render a hifitime [`Epoch`] to leap-correct UTC RFC 3339. `to_gregorian_utc`
/// applies the TAI→UTC leap-second conversion; formatting the components here
/// makes the output independent of hifitime's display time scale.
fn utc_rfc3339(epoch: Epoch) -> String {
    let (y, mo, d, h, mi, s, ns) = epoch.to_gregorian_utc();
    if ns == 0 {
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
    } else {
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ns:09}Z")
    }
}

/// Decode GPS system time (seconds since the GPS epoch 1980-01-06). GPS has no
/// internal leap seconds; GPS↔UTC uses the leap table (GPS−UTC = 18 s since
/// 2017-01-01).
#[must_use]
pub fn from_gps_seconds(seconds: f64) -> LeapReading {
    LeapReading {
        scale: "GPS",
        citation: "IS-GPS-200 (GPS system time; epoch 1980-01-06)",
        utc_rfc3339: utc_rfc3339(Epoch::from_gpst_seconds(seconds)),
        assumptions: vec![
            "consistent with GPS system time — a reading, not a determination".to_string(),
            "GPS↔UTC via the leap-second table; GPS itself has no leap seconds".to_string(),
        ],
    }
}

/// Decode a TAI64 external label (`2^62 + s`, where `s` = TAI seconds since
/// 1970-01-01 00:00:00 TAI; D. J. Bernstein libtai). TAI is leap-aware; the UTC
/// rendering subtracts the TAI−UTC offset.
pub fn from_tai64(label: u64) -> Result<LeapReading, ChronoError> {
    let s = label
        .checked_sub(TAI64_BASE)
        .ok_or(ChronoError::OutOfRange {
            what: "TAI64 label below 2^62 (pre-1970 era not supported)",
            value: i128::from(label),
        })?;
    let tai_since_1970 = i64::try_from(s).map_err(|_| ChronoError::OutOfRange {
        what: "TAI64 seconds offset too large",
        value: i128::from(s),
    })?;
    let secs_since_1900 =
        tai_since_1970
            .checked_add(SECS_1900_TO_1970)
            .ok_or(ChronoError::OutOfRange {
                what: "TAI seconds overflow",
                value: i128::from(tai_since_1970),
            })?;
    Ok(LeapReading {
        scale: "TAI64",
        citation: "D. J. Bernstein libtai TAI64 (label = 2^62 + TAI seconds since 1970)",
        utc_rfc3339: utc_rfc3339(Epoch::from_tai_seconds(secs_since_1900 as f64)),
        assumptions: vec![
            "consistent with a TAI64 label — a reading, not a determination".to_string(),
            "TAI→UTC applies the leap-second table (TAI−UTC = 37 s since 2017)".to_string(),
        ],
    })
}

/// Decode an NTP timestamp's seconds field (seconds since the 1900-01-01 prime
/// epoch). NTP follows UTC and does NOT count leap seconds, so the conversion is
/// additive (reused via `jiff`, not hifitime). Era 0 is assumed.
pub fn from_ntp_seconds(seconds: u64) -> Result<LeapReading, ChronoError> {
    let unix = i64::try_from(seconds)
        .map(|s| s - SECS_1900_TO_1970)
        .map_err(|_| ChronoError::OutOfRange {
            what: "NTP seconds too large",
            value: i128::from(seconds),
        })?;
    let ts = jiff::Timestamp::from_second(unix).map_err(|e| ChronoError::Render(e.to_string()))?;
    Ok(LeapReading {
        scale: "NTP",
        citation: "RFC 5905 (NTPv4; prime epoch 1900-01-01)",
        utc_rfc3339: ts.to_string(),
        assumptions: vec![
            "consistent with an NTP timestamp — a reading, not a determination".to_string(),
            "NTP follows UTC (no leap-second counting); conversion is additive".to_string(),
            "era 0 assumed — the 32-bit seconds field wraps in 2036; RFC 5905's \
             128-bit date format carries an explicit era to disambiguate"
                .to_string(),
        ],
    })
}

/// Dispatch a `--from <id>` for the leap family. Returns `None` when `id` is not
/// a leap scale, so the caller falls through to the POSIX registry.
#[must_use]
pub fn decode(id: &str, value: i64) -> Option<Result<LeapReading, ChronoError>> {
    match id {
        "gps" => Some(Ok(from_gps_seconds(value as f64))),
        "tai64" => Some(
            u64::try_from(value)
                .map_err(|_| ChronoError::OutOfRange {
                    what: "TAI64 label (negative)",
                    value: i128::from(value),
                })
                .and_then(from_tai64),
        ),
        "ntp" => Some(
            u64::try_from(value)
                .map_err(|_| ChronoError::OutOfRange {
                    what: "NTP seconds (negative)",
                    value: i128::from(value),
                })
                .and_then(from_ntp_seconds),
        ),
        _ => None,
    }
}
