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

/// Classify the transition that makes the wall time `naive` ambiguous, by whether
/// it was a daylight-saving change. `None` for UTC / fixed zones (which never
/// transition) and for a wall time that is not ambiguous.
///
/// `naive` is the wall-clock value with its civil fields read as UTC — the same
/// input [`resolve_local`] takes, NOT one of the two resolved instants. Taking
/// the last transition before a resolved instant would be wrong for the earlier
/// half of a fold: the transition that causes the ambiguity lies AFTER it, so
/// `preceding()` would return the previous, unrelated transition and report its
/// character instead.
#[must_use]
pub fn ambiguity_cause(naive: PosixNs, zone: &RenderZone) -> Option<AmbiguityCause> {
    let tz = match zone {
        RenderZone::Utc | RenderZone::Fixed(_) => return None,
        RenderZone::Named(tz) => tz,
    };
    let ts = jiff::Timestamp::from_nanosecond(naive.0).ok()?;
    let civil = jiff::tz::Offset::UTC.to_datetime(ts);
    let amb = tz.to_ambiguous_timestamp(civil);
    // The transition instant itself. For a fold it lies BETWEEN the two halves
    // (not at `later()`, which is the wall time re-expressed after the jump), so
    // it is found by stepping forward from the earlier half. For a gap it is the
    // first transition at or after the wall time.
    let at = match amb.offset() {
        jiff::tz::AmbiguousOffset::Fold { .. } => {
            let earlier =
                jiff::Timestamp::from_nanosecond(amb.earlier().ok()?.as_nanosecond()).ok()?;
            tz.following(earlier).next()?.timestamp().as_nanosecond()
        }
        jiff::tz::AmbiguousOffset::Gap { .. } => {
            // The gap's transition is the one that SKIPPED this wall time. The
            // skipped range is expressed in LOCAL time (Caracas 2016-05-01 skips
            // 02:30-03:00 local, which is 07:00-07:30Z), so compare the wall
            // time's civil fields against the local range either side of each
            // candidate transition — not against the UTC surrogate.
            let day = 86_400_000_000_000_i128;
            let probe = jiff::Timestamp::from_nanosecond(ts.as_nanosecond() - day).ok()?;
            let limit = jiff::Timestamp::from_nanosecond(ts.as_nanosecond() + day).ok()?;
            let mut found = None;
            for t in tz.following(probe) {
                if t.timestamp() > limit {
                    // cov:unreachable: jiff reported a Gap, so some transition's
                    // local before/after range contains `civil`; that transition
                    // lies within (civil - offset_after, civil - offset_before),
                    // and every tzdb offset is within ±1 day, so it is inside
                    // [probe, limit] and the loop breaks on it first. Kept so a
                    // pathological zone cannot spin.
                    break;
                }
                let prev =
                    jiff::Timestamp::from_nanosecond(t.timestamp().as_nanosecond() - 1).ok()?;
                // Local wall time immediately before and after the jump.
                let before_local = tz.to_offset(prev).to_datetime(prev);
                let after_local = t.offset().to_datetime(t.timestamp());
                if civil > before_local && civil < after_local {
                    found = Some(t.timestamp().as_nanosecond());
                    break;
                    // cov:unreachable: the no-match fall-through below — measured
                    // on tzdb 2026e, no forward gap occurs within 2 days of the
                    // preceding transition (2,403,715 adjacent pairs checked), and
                    // the gap's transition lies within ~1.6 days after `probe`, so
                    // the first transition examined is always the gap's. Kept: a
                    // future tzdb could add such a pair, and then continuing the
                    // search is the correct handling.
                }
            }
            found?
        }
        jiff::tz::AmbiguousOffset::Unambiguous { .. } => return None,
    };
    let before = jiff::Timestamp::from_nanosecond(at - 1).ok()?;
    let at_ts = jiff::Timestamp::from_nanosecond(at).ok()?;
    let was_dst = tz.to_offset_info(before).dst();
    Some(if was_dst == tz.to_offset_info(at_ts).dst() {
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
