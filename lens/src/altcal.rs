//! Hebrew + Islamic (tabular civil) calendar view for the overlay's calendar
//! expansion — the lens counterpart to the `cal` day card's alt-calendar rows.
//! Pure over the engine (the conversion + month names live in the `timeglyph`
//! library, shared with `cal`); the egui rendering is the shell.

use timeglyph::{calfmt, PosixNs, RenderZone};

/// The Hebrew and Islamic dates of an instant, formatted for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltCalView {
    /// e.g. `1 Tishrei 5768`.
    pub hebrew: String,
    /// e.g. `1 Ramadan 1428` (tabular civil).
    pub islamic: String,
}

/// The Hebrew + Islamic view for `instant` at the meridian `zone`. `None` if the
/// instant's civil date is out of the representable range.
#[must_use]
pub fn altcal_view(instant: PosixNs, zone: &RenderZone) -> Option<AltCalView> {
    let (hebrew, islamic) = timeglyph::cal::altcal_at(instant, zone);
    let (h, i) = (hebrew?, islamic?);
    Some(AltCalView {
        hebrew: format!(
            "{} {} {}",
            h.day,
            calfmt::hebrew_month(&h.month_code),
            h.year
        ),
        islamic: format!("{} {} {}", i.day, calfmt::islamic_month(i.month), i.year),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hebrew_and_islamic_of_a_known_date() {
        // 2007-09-13T12:00Z → 1 Tishrei 5768 (Rosh Hashanah) / 1 Ramadan 1428.
        let ns = 1_189_684_800_i128 * 1_000_000_000;
        let v = altcal_view(PosixNs(ns), &RenderZone::Utc).unwrap();
        assert_eq!(v.hebrew, "1 Tishrei 5768");
        assert_eq!(v.islamic, "1 Ramadan 1428");
    }
}
