//! The weekday / public-holiday labels must describe the date actually shown.
//!
//! `datetime_cell` renders the datetime from `Reading::instant` in the CURRENT display
//! zone, but the labels beside it were derived from the reading's baked `rendered`
//! string — so after a zone change that crosses a date boundary the card showed one
//! date and labelled it with another day's weekday and holidays. In a forensic tool
//! that is wrong output, not a cosmetic slip.
#![allow(clippy::unwrap_used)]

use timeglyph::{DateStyle, RenderZone};
use timeglyph_lens::scan;
use timeglyph_lens::text::label_basis;
use timeglyph_lens::zone::parse_zone;

/// The `unix` reading for a value, by format id — the top-ranked reading for these
/// fixtures is a naive wall-clock format, and naive readings deliberately do not
/// follow the display zone, so only a zone-anchored one can show the defect.
fn unix_reading(text: &str) -> scan::Reading {
    scan::inspect_text(text, 8, &RenderZone::Utc)
        .into_iter()
        .flat_map(|h| h.readings)
        .find(|r| r.format_id == "unix")
        .expect("a unix reading")
}

#[test]
fn a_zone_anchored_label_follows_the_display_zone() {
    let r = unix_reading("1721000000");
    // Baseline: UTC puts this instant on the 14th, a Sunday.
    let utc = label_basis(&r, &RenderZone::Utc).unwrap();
    assert!(utc.starts_with("2024-07-14"), "UTC basis: {utc}");
    assert_eq!(scan::weekday(&utc), Some("Sunday"));

    // Asia/Tokyo puts the SAME instant on the 15th, a Monday. The basis must move
    // with it, or the label contradicts the date in the cell.
    let tokyo = parse_zone("Asia/Tokyo").unwrap();
    let tk = label_basis(&r, &tokyo.zone).unwrap();
    assert!(tk.starts_with("2024-07-15"), "Tokyo basis: {tk}");
    assert_eq!(
        scan::weekday(&tk),
        Some("Monday"),
        "the weekday must describe the date the cell shows"
    );
}

#[test]
fn a_naive_reading_never_shifts_with_the_zone() {
    // A local wall-clock value carries no offset; shifting it would fabricate
    // meaning, so its label basis stays put whatever the display zone says.
    let naive = scan::inspect_text("1721000000", 8, &RenderZone::Utc)
        .into_iter()
        .flat_map(|h| h.readings)
        .find(|r| r.local)
        .expect("a local-naive reading (exfat/fat) for this value");
    let tokyo = parse_zone("Asia/Tokyo").unwrap();
    let a = label_basis(&naive, &RenderZone::Utc).unwrap();
    let b = label_basis(&naive, &tokyo.zone).unwrap();
    assert_eq!(a, b, "a naive reading's basis is zone-independent");
}

#[test]
fn a_naive_basis_is_iso_even_when_the_reading_was_decoded_under_another_style() {
    // Replaces an earlier version of this test that could not fail: it looped four
    // DateStyles over a NON-local reading, whose branch is `instant.render(zone)` and
    // therefore style-independent — all four iterations computed the same string, and
    // it never touched the branch that actually breaks.
    //
    // The real risk is the LOCAL branch. A naive reading's `rendered` is baked at
    // decode time in whatever style was used, and `scan::weekday` / the holiday lookup
    // parse `YYYY-MM-DD` from the front — so returning that string would make the
    // labels VANISH under a non-ISO style instead of being correct. Decoding with
    // `inspect_text_opts` lets the style actually reach `rendered`, which
    // `scan::inspect_text` (hard-coded to Iso8601) cannot.
    for style in [
        DateStyle::Iso8601,
        DateStyle::SpaceSeparated,
        DateStyle::Rfc2822,
        DateStyle::UsStyle,
    ] {
        let naive = scan::inspect_text_opts("1721000000", 8, 8, false, &RenderZone::Utc, style)
            .into_iter()
            .flat_map(|h| h.readings)
            .find(|r| r.local)
            .expect("a local-naive reading (exfat/fat) for this value");
        let basis =
            label_basis(&naive, &RenderZone::Utc).expect("a rendered naive reading has a basis");
        assert_eq!(
            scan::weekday(&basis),
            Some("Sunday"),
            "the basis must stay ISO-parseable when decoded under {style:?}; \
             got basis={basis:?} (rendered={:?})",
            naive.rendered
        );
    }
}

#[test]
fn the_label_basis_always_agrees_with_the_datetime_the_cell_shows() {
    // The invariant the whole fix exists to hold: whatever date the cell displays,
    // the weekday / holiday labels describe THAT date. Checked across every reading
    // of several values (zone-anchored, naive and offset-embedded alike) in a zone
    // that crosses a date boundary relative to UTC.
    //
    // Not a RED-first test: it pins an invariant the fix already satisfies, as a
    // guard against future drift between `copy_text_for` (what is shown) and
    // `label_basis` (what is labelled) — the two branched identically on `r.local`,
    // and nothing but a test stops them diverging.
    let tokyo = parse_zone("Asia/Tokyo").unwrap();
    for value in [
        "1721000000",                // unix seconds + the naive packed formats
        "133801920000000000",        // FILETIME
        "2024-07-14T23:33:20+08:00", // offset-embedded string form
    ] {
        for zone in [&RenderZone::Utc, &tokyo.zone] {
            for hit in scan::inspect_text(value, 8, zone) {
                for r in &hit.readings {
                    let shown = timeglyph_lens::text::copy_text_for(r, zone, DateStyle::Iso8601);
                    let basis = label_basis(r, zone).expect("a rendered reading has a label basis");
                    assert_eq!(
                        shown.get(..10),
                        basis.get(..10),
                        "label basis must match the shown date for {} ({value}): \
                         shown={shown} basis={basis}",
                        r.format_id
                    );
                }
            }
        }
    }
}
