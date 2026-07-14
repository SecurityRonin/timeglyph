//! Alternative-calendar view for the overlay's calendar expansion — the lens
//! counterpart to the `cal` day card's alt-calendar rows. Pure over the engine
//! (the conversion + names live in the `timeglyph` library, shared with `cal`);
//! the egui rendering is the shell.

use timeglyph::{PosixNs, RenderZone};

/// The alternative calendars of an instant, formatted for display, in order
/// (中華民國 · Japanese · Buddhist · Hebrew · Islamic · Persian).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltCalView {
    /// Each calendar as `(name, formatted)`.
    pub calendars: Vec<(String, String)>,
}

/// The alternative-calendar view for `instant` at the meridian `zone`. `None` if
/// the instant's civil date is out of the representable range.
#[must_use]
pub fn altcal_view(instant: PosixNs, zone: &RenderZone) -> Option<AltCalView> {
    let calendars: Vec<(String, String)> = timeglyph::cal::extra_calendars_at(instant, zone)
        .into_iter()
        .map(|e| (e.name, e.formatted))
        .collect();
    if calendars.is_empty() {
        return None;
    }
    Some(AltCalView { calendars })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_calendars_in_order() {
        // 2007-09-13T12:00Z. Order: 中華民國 · Japanese · Buddhist · Hebrew ·
        // Islamic · Persian.
        let ns = 1_189_684_800_i128 * 1_000_000_000;
        let v = altcal_view(PosixNs(ns), &RenderZone::Utc).unwrap();
        let names: Vec<&str> = v.calendars.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "中華民國",
                "Japanese",
                "Buddhist",
                "Hebrew",
                "Islamic",
                "Persian"
            ]
        );
        let by = |n: &str| &v.calendars.iter().find(|(x, _)| x == n).unwrap().1;
        assert_eq!(by("Hebrew"), "1 Tishrei 5768");
        assert_eq!(by("Islamic"), "1 Ramadan 1428");
        assert!(by("中華民國").contains("年"));
    }
}
