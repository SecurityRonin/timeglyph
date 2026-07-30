//! Tests for decoding the clipboard — the path that works when hovering cannot
//! reach the value, above all inside a VM guest (the host accessibility tree stops
//! at the guest window, but the clipboard crosses that boundary; verified).
//!
//! The clipboard is injected behind a trait so these never touch the real system
//! clipboard: doing so would be flaky and would clobber the developer's clipboard.
#![allow(clippy::unwrap_used)]

use timeglyph_lens::clipboard::{read_decodable, source_caption, ClipboardRead, SourceContext};

/// A fake clipboard. Tests own the content, so nothing reads the real pasteboard.
struct Fake(Option<&'static str>);

impl ClipboardRead for Fake {
    fn text(&mut self) -> Option<String> {
        self.0.map(str::to_owned)
    }
}

#[test]
fn reads_text_from_the_clipboard() {
    let mut c = Fake(Some("133801920000000000"));
    assert_eq!(
        read_decodable(&mut c).as_deref(),
        Some("133801920000000000")
    );
}

#[test]
fn surrounding_whitespace_is_trimmed() {
    // Copying from a table or a log usually drags padding along.
    let mut c = Fake(Some("  1577836800\n"));
    assert_eq!(read_decodable(&mut c).as_deref(), Some("1577836800"));
}

#[test]
fn empty_or_blank_clipboard_yields_nothing() {
    assert_eq!(read_decodable(&mut Fake(None)), None);
    assert_eq!(read_decodable(&mut Fake(Some(""))), None);
    assert_eq!(
        read_decodable(&mut Fake(Some("   \t\n"))),
        None,
        "blank is not a value"
    );
}

#[test]
fn clipboard_source_can_never_expose_raw_text() {
    // THE privacy property, enforced by the type rather than by a conditional: the
    // overlay renders the source caption in an always-on-top window, so a clipboard
    // variant that could carry text would eventually put a copied password on
    // screen. `SourceContext::Clipboard` holds no text at all, so there is nothing
    // to leak — one missed branch cannot regress it.
    assert_eq!(source_caption(&SourceContext::Clipboard), None);
}

#[test]
fn cursor_source_still_captions_its_element_text() {
    let s = SourceContext::Cursor("created 133801920000000000".to_owned());
    assert_eq!(source_caption(&s), Some("created 133801920000000000"));
}
