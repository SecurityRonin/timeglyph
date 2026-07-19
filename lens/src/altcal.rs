//! Alternative-calendar view for the overlay's calendar expansion — the lens
//! counterpart to the `cal` day card's alt-calendar rows. Pure over the engine
//! (the conversion + names live in the `timeglyph` library, shared with `cal`);
//! the egui rendering is the shell.

use timeglyph::{PosixNs, RenderZone};

/// One alternative-calendar row: its stable `key` (for the visibility toggle) and
/// the ready-to-show `label` (display name + formatted date).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltCalRow {
    /// Stable toggle key (`roc`, `japanese`, …).
    pub key: String,
    /// The display line, e.g. `中華民國 Republic of China 113年5月13日`.
    pub label: String,
}

/// The alternative calendars of an instant, in order (中華民國 · 和暦 · Buddhist ·
/// Hebrew · Islamic · Persian).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltCalView {
    /// The rows, in display order.
    pub calendars: Vec<AltCalRow>,
}

/// The alternative-calendar view for `instant` at the meridian `zone`. `None` if
/// the instant's civil date is out of the representable range.
#[must_use]
pub fn altcal_view(instant: PosixNs, zone: &RenderZone) -> Option<AltCalView> {
    let calendars: Vec<AltCalRow> = timeglyph::cal::extra_calendars_at(instant, zone)
        .into_iter()
        .map(|e| AltCalRow {
            key: e.key,
            label: format!("{} {}", e.name, e.formatted),
        })
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
    fn all_six_calendars_in_order_with_keys() {
        // 2007-09-13T12:00Z. Order by stable key: roc · japanese · buddhist ·
        // hebrew · islamic · persian.
        let ns = 1_189_684_800_i128 * 1_000_000_000;
        let v = altcal_view(PosixNs(ns), &RenderZone::Utc).unwrap();
        let keys: Vec<&str> = v.calendars.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(
            keys,
            ["roc", "japanese", "buddhist", "hebrew", "islamic", "persian"]
        );
        let by = |k: &str| &v.calendars.iter().find(|r| r.key == k).unwrap().label;
        // Bilingual label: native name + English + the formatted date.
        assert!(by("hebrew").contains("Hebrew") && by("hebrew").contains("1 Tishrei 5768"));
        assert!(by("islamic").contains("1 Ramadan 1428"));
        assert!(by("roc").contains("中華民國") && by("roc").contains("年"));
    }
}
