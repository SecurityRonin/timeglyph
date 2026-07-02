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
            Some(ZoneStamp {
                offset: info.offset().to_string(),
                abbr: info.abbreviation().to_string(),
                dst: matches!(info.dst(), jiff::tz::Dst::Yes),
            })
        }
    }
}
