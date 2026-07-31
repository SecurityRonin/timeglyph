//! A clipboard press must report what it found — the three outcomes have to be
//! distinguishable by the caller.
//!
//! Why this matters. `decode` used to answer `Option`, collapsing "the clipboard was
//! empty" and "the clipboard held text but none of it decoded" into the same `None`,
//! and the overlay treated that as *leave the display alone*. The intent was
//! protective: a misfired press should not wipe the reading you were studying. The
//! effect was that a deliberate user action produced no visible result at all, while
//! the previous reading — possibly a hovered value, with that hovered text still
//! showing as the source caption — stayed on screen. Copy a password, press the
//! button, and the panel shows a timestamp and a caption with nothing to say the
//! press found nothing. The stale reading reads as the answer to what you just
//! copied.
//!
//! So the outcome is now explicit, and carries a fixed notice the overlay can draw as
//! provenance for the miss.
#![allow(clippy::unwrap_used)]

use timeglyph::RenderZone;
use timeglyph_lens::clipboard::{decode, ClipboardOutcome, ClipboardRead, SourceContext};

/// A fake clipboard. Tests own the content, so nothing reads the real pasteboard.
struct Fake(Option<&'static str>);

impl ClipboardRead for Fake {
    fn text(&mut self) -> Option<String> {
        self.0.map(str::to_owned)
    }
}

fn outcome(content: Option<&'static str>) -> ClipboardOutcome {
    let mut c = Fake(content);
    decode(&mut c, 8, &RenderZone::Utc)
}

#[test]
fn an_empty_clipboard_is_distinct_from_one_that_decoded_nothing() {
    // The whole point: these were both `None` before, so the overlay could not tell
    // them apart and said nothing for either.
    assert!(
        matches!(outcome(None), ClipboardOutcome::Empty),
        "no text at all must report Empty"
    );
    assert!(
        matches!(
            outcome(Some("no timestamps live here")),
            ClipboardOutcome::NothingDecoded
        ),
        "text that yields no readings must report NothingDecoded, not Empty"
    );
}

#[test]
fn whitespace_only_counts_as_empty_not_as_a_failed_decode() {
    assert!(
        matches!(outcome(Some("   \n\t  ")), ClipboardOutcome::Empty),
        "padding copied from a table is nothing, not a decode failure"
    );
}

#[test]
fn a_decodable_value_still_decodes_and_is_sourced_from_the_clipboard() {
    let ClipboardOutcome::Decoded(source, hits) = outcome(Some("1721000000")) else {
        panic!("a unix timestamp must decode");
    };
    assert_eq!(source, SourceContext::Clipboard);
    assert!(!hits.is_empty(), "a decode must carry readings");
}

#[test]
fn every_miss_carries_a_notice_and_a_decode_carries_none() {
    // The notice is what breaks the false association: it gives the press a visible
    // result, so the readings still on screen cannot be read as its answer.
    assert!(
        outcome(None).notice().is_some(),
        "an empty clipboard must give the press a visible result"
    );
    assert!(
        outcome(Some("nothing here")).notice().is_some(),
        "a failed decode must give the press a visible result"
    );
    assert!(
        outcome(Some("1721000000")).notice().is_none(),
        "a successful decode speaks through its readings, not a notice"
    );
}

#[test]
fn a_notice_never_contains_clipboard_content() {
    // The safety property. The notice is drawn in an always-on-top window, so it must
    // be a fixed string — never an echo of what was copied. A secret that fails to
    // decode must not reach the screen by way of the error path.
    const SECRET: &str = "hunter2-correct-horse-battery-staple";
    let n = outcome(Some(SECRET))
        .notice()
        .expect("a miss carries a notice");
    assert!(
        !n.contains("hunter2") && !n.contains(SECRET),
        "the notice leaked clipboard content: {n:?}"
    );
    // And it must not leak a non-decoding numeric secret either.
    const NUMERIC: &str = "9999999999999999999999999";
    if let Some(n) = outcome(Some(NUMERIC)).notice() {
        assert!(
            !n.contains(NUMERIC),
            "the notice leaked a numeric secret: {n:?}"
        );
    }
}
