//! The lens obeys the show-all principle: it displays every confident (in-window)
//! reading for a number, scrollable — likelihood ranks them, it never filters to
//! a top-N slice. Guards against silently re-capping the overlay to a few rows.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::{scan, RenderZone};

#[test]
fn lens_reading_cap_shows_all_confident_readings() {
    // 1300000000 decodes in-window under 9 formats (exfat/fat/unix/dvr/iostime/
    // postgres/sony/discord/snowflake). The lens cap must surface all of them,
    // not a top-4 slice.
    let hits = scan::inspect_text(
        "1300000000",
        timeglyph_lens::READINGS_SHOWN,
        &RenderZone::Utc,
    );
    let n = hits
        .iter()
        .find(|h| h.number == "1300000000")
        .expect("number scanned")
        .readings
        .len();
    assert!(
        n > 4,
        "the lens cap must not hide readings (show-all): got {n}, expected all confident"
    );
}
