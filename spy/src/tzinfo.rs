//! Per-instant zone stamping: the offset, abbreviation, and DST status a named
//! zone resolves to *at a given instant*. A location alone is ambiguous
//! (Europe/London is GMT in winter, BST in summer), so the stamp is always tied
//! to a specific instant. Pure and testable.

use timeglyph::{PosixNs, RenderZone};

/// A zone's resolved presentation at one instant.
#[derive(Debug, Clone)]
pub struct ZoneStamp {
    /// Numeric UTC offset, e.g. `-04:00`.
    pub offset: String,
    /// Zone abbreviation for this instant, e.g. `EDT`/`EST`/`GMT`/`BST`. Empty
    /// for a fixed offset (there is no named rule to abbreviate).
    pub abbr: String,
    /// Whether daylight saving time is in effect at this instant.
    pub dst: bool,
}

/// Central meridian (degrees east) of a UTC offset in hours — offset × 15°. The
/// single offset→meridian formula, shared by [`meridian_longitude`] and the map.
#[must_use]
pub fn meridian_of_offset(hours: f64) -> f64 {
    hours * 15.0
}

/// The central meridian of `zone` at `instant`, in degrees east — its *standard*
/// UTC offset × 15°. DST is removed (a zone's geography doesn't shift in summer),
/// so New York is `-75.0` in both seasons and London `0.0` whether GMT or BST.
/// Used to default the 干支 longitude when a location is picked. `None` only if
/// `instant` is out of range.
#[must_use]
pub fn meridian_longitude(zone: &RenderZone, instant: PosixNs) -> Option<f64> {
    let hours = match zone {
        RenderZone::Utc => 0.0,
        RenderZone::Fixed(off) => f64::from(off.seconds()) / 3600.0,
        RenderZone::Named(tz) => {
            let ts = jiff::Timestamp::from_nanosecond(instant.0).ok()?;
            let info = tz.to_offset_info(ts);
            let mut h = f64::from(info.offset().seconds()) / 3600.0;
            if matches!(info.dst(), jiff::tz::Dst::Yes) {
                h -= 1.0; // standard offset — the meridian is geographic, not clock
            }
            h
        }
    };
    Some(meridian_of_offset(hours))
}

/// Resolve the [`ZoneStamp`] for `instant` in `zone`. `None` for [`RenderZone::Utc`]
/// (readings already show `Z` — nothing to add) or an out-of-range instant.
#[must_use]
pub fn stamp(zone: &RenderZone, instant: PosixNs) -> Option<ZoneStamp> {
    let ts = jiff::Timestamp::from_nanosecond(instant.0).ok()?;
    match zone {
        RenderZone::Utc => None,
        RenderZone::Fixed(off) => Some(ZoneStamp {
            offset: off.to_string(),
            abbr: String::new(),
            dst: false,
        }),
        RenderZone::Named(tz) => {
            let info = tz.to_offset_info(ts);
            // A numeric "abbreviation" (e.g. Acre's `-05`) just repeats the
            // offset, so drop it — only a real letter code (EST/GMT/BST) adds
            // information beyond the numeric offset already shown.
            let raw = info.abbreviation();
            let abbr = if raw.starts_with(['+', '-']) {
                String::new()
            } else {
                raw.to_string()
            };
            Some(ZoneStamp {
                offset: info.offset().to_string(),
                abbr,
                dst: matches!(info.dst(), jiff::tz::Dst::Yes),
            })
        }
    }
}
