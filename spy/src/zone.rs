//! Display-zone parsing for the overlay's footer control. Pure and testable; the
//! egui footer is the thin shell over this.

use timeglyph::RenderZone;

/// A parsed display zone plus how to present it in the footer chip.
#[derive(Debug, Clone)]
pub struct ZoneChoice {
    /// The zone readings are rendered in.
    pub zone: RenderZone,
    /// Short label for the footer chip (`UTC`, `Local`, `Asia/Shanghai`, …).
    pub label: String,
    /// True for any non-UTC zone. The footer renders a loud zone prominently so a
    /// glance cannot mistake the frame for UTC (a classic timestamp-reading error).
    pub loud: bool,
}

impl Default for ZoneChoice {
    fn default() -> Self {
        Self {
            zone: RenderZone::Utc,
            label: "UTC".to_string(),
            loud: false,
        }
    }
}

/// Parse a zone spec into a [`ZoneChoice`], or `None` if unrecognised.
///
/// `""` / `UTC` / `Z` → UTC (the calm default); `local` / `system` → the host's
/// zone; a leading `+`/`-` → a fixed offset; anything else → an IANA name,
/// validated against the tz database (an unknown name is rejected, never a silent
/// UTC fallback).
#[must_use]
pub fn parse_zone(input: &str) -> Option<ZoneChoice> {
    let s = input.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("utc") || s.eq_ignore_ascii_case("z") {
        return Some(ZoneChoice::default());
    }
    if s.eq_ignore_ascii_case("local") || s.eq_ignore_ascii_case("system") {
        return Some(ZoneChoice {
            zone: RenderZone::Named(jiff::tz::TimeZone::system()),
            label: "Local".to_string(),
            loud: true,
        });
    }
    let zone = RenderZone::parse(s).ok()?;
    Some(ZoneChoice {
        zone,
        label: s.to_string(),
        loud: true,
    })
}
