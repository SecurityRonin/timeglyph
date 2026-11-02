//! Interpreting a `LocalNaive` wall-clock value *in* a concrete zone.
//!
//! A `LocalNaive` format (FAT, EXIF, DOSDATE…) stores civil wall-clock with no
//! offset; the engine decodes it to a *naive* instant (the civil fields read as
//! UTC). Placing those civil fields in a real zone is where DST ambiguity appears:
//! a fall-back **fold** maps one wall time to two instants, a spring-forward
//! **gap** to none. This is a property of the IANA tzdb — computed here via jiff's
//! ambiguous-timestamp API, never guessed. A gap is itself a lead: a correctly
//! clocked device in that zone cannot have written a nonexistent wall time.

use crate::{PosixNs, RenderZone};

/// Whether the transition that makes a wall time ambiguous was a daylight-saving
/// change or a standard-offset move.
///
/// A fold or gap can be produced by either: Venezuela's 2007 move from -04:00 to
/// -04:30 repeats a half hour of wall time without any daylight saving being
/// involved. The ambiguity is the same, but attributing it to DST would assert a
/// transition that never happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbiguityCause {
    /// The tzdb's DST flag flipped across the transition.
    DaylightSaving,
    /// The offset moved but the DST flag did not: a jurisdiction redefining its
    /// standard offset.
    StandardOffset,
}

/// Classify the transition nearest `instant` in `zone` by whether it was a
/// daylight-saving change. `None` for UTC / fixed zones, which never transition.
///
/// Looks at the transition at or before `instant` and compares the tzdb's DST
/// flag on each side of it.
#[must_use]
pub fn ambiguity_cause(instant: PosixNs, zone: &RenderZone) -> Option<AmbiguityCause> {
    let tz = match zone {
        RenderZone::Utc | RenderZone::Fixed(_) => return None,
        RenderZone::Named(tz) => tz,
    };
    let ts = jiff::Timestamp::from_nanosecond(instant.0).ok()?;
    // The transition governing this instant is the last one at or before it.
    let t = tz.preceding(ts).next()?;
    let before = jiff::Timestamp::from_nanosecond(t.timestamp().as_nanosecond() - 1).ok()?;
    let was_dst = tz.to_offset_info(before).dst();
    Some(if was_dst == t.dst() {
        AmbiguityCause::StandardOffset
    } else {
        AmbiguityCause::DaylightSaving
    })
}

/// How a `LocalNaive` wall time resolves to an absolute instant in a given zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalResolution {
    /// Exactly one instant — the common case, and always so for UTC / fixed zones.
    Unique(PosixNs),
    /// A fall-back **fold**: the wall time occurred twice. `earlier` is the
    /// pre-transition instant (the larger UTC offset, e.g. EDT), `later` the
    /// post-transition one (the smaller offset, e.g. EST).
    Fold {
        /// The earlier instant (pre-transition offset).
        earlier: PosixNs,
        /// The later instant (post-transition offset).
        later: PosixNs,
    },
    /// A spring-forward **gap**: the wall time never existed in this zone.
    Gap,
}

/// Resolve a naive wall-clock instant (civil fields read as UTC) as if those civil
/// fields were wall time *in* `zone`.
///
/// - `Utc` → the value is already the instant (unique).
/// - `Fixed(offset)` → the civil time at that offset (unique; fixed offsets never
///   have DST).
/// - `Named(tz)` → per the tzdb: `Unique`, `Fold` (fall-back), or `Gap`
///   (spring-forward).
///
/// Never panics: an out-of-range instant or an internal jiff error degrades to
/// `Unique(naive)` rather than fabricating a fold/gap.
#[must_use]
pub fn resolve_local(naive: PosixNs, zone: &RenderZone) -> LocalResolution {
    let tz = match zone {
        RenderZone::Utc => return LocalResolution::Unique(naive),
        RenderZone::Fixed(offset) => offset.to_time_zone(),
        RenderZone::Named(tz) => tz.clone(),
    };
    let Ok(ts) = jiff::Timestamp::from_nanosecond(naive.0) else {
        return LocalResolution::Unique(naive);
    };
    // The naive instant's civil fields, read in UTC, ARE the wall time to place.
    let civil = jiff::tz::Offset::UTC.to_datetime(ts);
    let amb = tz.to_ambiguous_timestamp(civil);
    match amb.offset() {
        jiff::tz::AmbiguousOffset::Unambiguous { .. } => amb
            .unambiguous()
            .map_or(LocalResolution::Unique(naive), |t| {
                LocalResolution::Unique(PosixNs(t.as_nanosecond()))
            }),
        jiff::tz::AmbiguousOffset::Gap { .. } => LocalResolution::Gap,
        jiff::tz::AmbiguousOffset::Fold { .. } => match (amb.earlier(), amb.later()) {
            (Ok(e), Ok(l)) => LocalResolution::Fold {
                earlier: PosixNs(e.as_nanosecond()),
                later: PosixNs(l.as_nanosecond()),
            },
            // cov:unreachable: jiff reports `AmbiguousOffset::Fold` only for a
            // civil time it can resolve at BOTH offsets, so `earlier()`/`later()`
            // cannot fail here. Kept so an internal jiff error degrades to the
            // naive instant rather than fabricating a fold.
            _ => LocalResolution::Unique(naive),
        },
    }
}
