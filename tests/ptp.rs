//! IEEE 1588 PTP timestamp (default profile). PTP counts seconds+nanoseconds
//! since the epoch 1970-01-01 on the TAI scale, so the UTC rendering subtracts the
//! TAI−UTC offset (37 s since 2017). Tier-1: the PTP epoch + TAI scale are the
//! spec's; TAI−UTC is the IERS table (via hifitime).
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(feature = "leap")]

use timeglyph::leap::from_ptp;

#[test]
fn ptp_default_profile_decodes_on_the_tai_scale() {
    // seconds = 1_577_836_800 → 2020-01-01T00:00:00 TAI; UTC = TAI − 37 s in 2020
    // = 2019-12-31T23:59:23.
    let r = from_ptp(1_577_836_800, 0);
    assert!(
        r.utc_rfc3339.starts_with("2019-12-31T23:59:23"),
        "PTP on the TAI scale: {}",
        r.utc_rfc3339
    );
}

#[test]
fn ptp_states_the_profile_assumption() {
    // Codex caveat: PTP is not always TAI/1970 — the profile (ptpTimescale, epoch)
    // matters, so the reading must state the assumption.
    let r = from_ptp(1_577_836_800, 500_000_000);
    assert!(r
        .assumptions
        .iter()
        .any(|a| a.to_lowercase().contains("profile")));
}
