//! "Explain this reading" — a per-format spec card generated from the registry
//! (epoch, unit, tz/leap, valid range, known sentinels, citation), so it can't
//! drift from the actual decoder. Registry-derived: the card is a projection of
//! the same Format the engine decodes with.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret::explain;

#[test]
fn explain_filetime_renders_a_spec_card() {
    let card = explain("filetime").expect("filetime is a registered format");
    let lc = card.to_lowercase();
    assert!(card.contains("filetime"), "names the format: {card}");
    assert!(card.contains("1601"), "shows the 1601 epoch: {card}");
    assert!(card.contains("100"), "shows the 100 ns tick unit: {card}");
    assert!(lc.contains("utc"), "shows the timezone semantics: {card}");
    assert!(
        lc.contains("citation") || card.contains("FILETIME"),
        "carries the citation: {card}"
    );
}

#[test]
fn explain_a_seconds_epoch_shows_its_epoch_instant() {
    // unix epoch is 1970-01-01; the card renders value 0.
    let card = explain("unix").expect("unix exists");
    assert!(card.contains("1970-01-01"), "unix epoch shown: {card}");
}

#[test]
fn explain_unknown_format_is_none() {
    assert!(explain("no-such-format").is_none());
}
