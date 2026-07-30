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
fn the_label_basis_is_iso_parseable_for_every_reading_and_decode_style() {
    // Claim 1 of 2, checked across every style: whatever style baked the reading, the
    // basis must remain ISO-parseable, because `scan::weekday` and the holiday lookup
    // read `YYYY-MM-DD` off the front. This is the property the naive-branch fix
    // delivers — it genuinely fails on the pre-fix code (Rfc2822 gave
    // "Sun, 20 Apr 2031 12:02:00", weekday None).
    //
    // A seen-counter guards against the loop passing VACUOUSLY if a fixture ever stops
    // yielding readings (review caught that in the earlier version).
    //
    // Deliberately makes no claim about `TzSemantics::OffsetEmbedded`: a `Reading`
    // carries only `local`, not tz semantics, so a test at this level structurally
    // cannot distinguish that case.
    let tokyo = parse_zone("Asia/Tokyo").unwrap();
    let mut seen = 0usize;
    for value in [
        "1721000000",
        "133801920000000000",
        "2024-07-14T23:33:20+08:00",
    ] {
        for zone in [&RenderZone::Utc, &tokyo.zone] {
            for style in [
                DateStyle::Iso8601,
                DateStyle::SpaceSeparated,
                DateStyle::Rfc2822,
                DateStyle::UsStyle,
            ] {
                for hit in scan::inspect_text_opts(value, 8, 8, false, zone, style) {
                    for r in &hit.readings {
                        let basis = label_basis(r, zone).expect("a rendered reading has a basis");
                        assert!(
                            scan::weekday(&basis).is_some(),
                            "basis must stay ISO-parseable however the reading was decoded \
                             ({} / {style:?}): basis={basis} rendered={:?}",
                            r.format_id,
                            r.rendered
                        );
                        seen += 1;
                    }
                }
            }
        }
    }
    assert!(
        seen > 50,
        "the loop must actually exercise readings, not pass vacuously; saw {seen}"
    );
}

#[test]
fn the_label_basis_names_the_same_date_the_cell_shows_at_iso() {
    // Claim 2 of 2, scoped honestly to Iso8601. The earlier version asserted
    // `shown.get(..10) == basis.get(..10)` for every style, which review showed is
    // false by design off ISO — and not because the labels are wrong: under Rfc2822 a
    // naive cell reads "Sun, 20 Apr 2031 12:02:00" while the basis is
    // "2031-04-20T12:02:00". Same date, different shape, so the prefixes cannot match.
    // (`copy_text_for` also ignores `style` for a local reading, returning the baked
    // string, so there is no style-independent reference to compare against either.)
    //
    // At Iso8601 both are ISO and the comparison is meaningful — which is the case the
    // lens actually ships, since `scan::inspect_text` hard-codes Iso8601.
    let tokyo = parse_zone("Asia/Tokyo").unwrap();
    let mut seen = 0usize;
    for value in [
        "1721000000",
        "133801920000000000",
        "2024-07-14T23:33:20+08:00",
    ] {
        for zone in [&RenderZone::Utc, &tokyo.zone] {
            for hit in scan::inspect_text(value, 8, zone) {
                for r in &hit.readings {
                    let shown = timeglyph_lens::text::copy_text_for(r, zone, DateStyle::Iso8601);
                    let basis = label_basis(r, zone).expect("a rendered reading has a basis");
                    assert_eq!(
                        shown.get(..10),
                        basis.get(..10),
                        "the label must describe the date the cell shows for {} ({value}): \
                         shown={shown} basis={basis}",
                        r.format_id
                    );
                    seen += 1;
                }
            }
        }
    }
    assert!(
        seen > 20,
        "must exercise readings, not pass vacuously; saw {seen}"
    );
}
