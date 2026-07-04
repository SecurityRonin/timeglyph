//! Datetime *display styles* shared by the CLI and the timeglyph-lens overlay.
//!
//! An instant ([`PosixNs`]) is absolute; a [`RenderZone`] fixes which civil
//! offset it is shown in; a [`DateStyle`] fixes the textual shape. Only the
//! shape changes here — the instant and the zone semantics are untouched.
//!
//! [`DateStyle::Iso8601`] is the crate default and reproduces
//! [`PosixNs::render`] byte for byte (offset / `Z` included); the other styles
//! render via jiff's strftime on the zoned instant.

use crate::{PosixNs, RenderZone};

/// A textual style for rendering an instant. [`Iso8601`](DateStyle::Iso8601) is
/// the default and matches the crate's canonical RFC 3339 rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum DateStyle {
    /// RFC 3339 / ISO 8601, e.g. `2020-01-01T00:00:00Z` — the canonical default.
    #[default]
    Iso8601,
    /// Space-separated civil datetime with a zone abbreviation, e.g.
    /// `2020-01-01 00:00:00 UTC`.
    SpaceSeparated,
    /// RFC 2822, e.g. `Wed, 01 Jan 2020 00:00:00 +0000`.
    Rfc2822,
    /// US-locale 12-hour clock, e.g. `01/01/2020 12:00:00 AM UTC`.
    UsStyle,
}

/// Format `instant` in `zone` using the given display `style`.
///
/// [`Iso8601`](DateStyle::Iso8601) delegates to [`PosixNs::render`] so it is an
/// exact reproduction of the crate's canonical rendering. The other styles zone
/// the instant and apply the matching jiff strftime pattern. An instant outside
/// jiff's civil range degrades to `<out of civil range>` rather than panicking.
#[must_use]
pub fn format_instant(instant: PosixNs, zone: &RenderZone, style: DateStyle) -> String {
    const OUT_OF_RANGE: &str = "<out of civil range>";

    // `Iso8601` is defined as an exact reproduction of the canonical rendering.
    let pattern = match style {
        DateStyle::Iso8601 => return instant.render(zone).unwrap_or_else(|| OUT_OF_RANGE.into()),
        DateStyle::SpaceSeparated => "%Y-%m-%d %H:%M:%S %Z",
        DateStyle::Rfc2822 => "%a, %d %b %Y %H:%M:%S %z",
        DateStyle::UsStyle => "%m/%d/%Y %I:%M:%S %p %Z",
    };

    let Ok(ts) = jiff::Timestamp::from_nanosecond(instant.0) else {
        return OUT_OF_RANGE.into();
    };
    let tz = match zone {
        RenderZone::Utc => jiff::tz::TimeZone::UTC,
        RenderZone::Fixed(offset) => jiff::tz::TimeZone::fixed(*offset),
        RenderZone::Named(tz) => tz.clone(),
    };
    ts.to_zoned(tz).strftime(pattern).to_string()
}
