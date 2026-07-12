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
            _ => LocalResolution::Unique(naive),
        },
    }
}
