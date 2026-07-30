//! Tests for decoding the clipboard — the path that works when hovering cannot
//! reach the value, above all inside a VM guest (the host accessibility tree stops
//! at the guest window, but the clipboard crosses that boundary; verified).
//!
//! The clipboard is injected behind a trait so these never touch the real system
//! clipboard: doing so would be flaky and would clobber the developer's clipboard.
#![allow(clippy::unwrap_used)]

use timeglyph_lens::clipboard::{
    caption, read_decodable, source_caption, ClipboardRead, SourceContext, CAPTION_MAX_CHARS,
};

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

#[test]
fn cursor_caption_collapses_runs_of_whitespace() {
    // The header renders one line, so the hovered element's newlines and tab
    // padding collapse to single spaces before display.
    let s = SourceContext::Cursor("  created\t\n 133801920000000000  ".to_owned());
    assert_eq!(caption(&s).as_deref(), Some("created 133801920000000000"));
}

#[test]
fn cursor_caption_is_bounded_at_the_display_limit() {
    let s = SourceContext::Cursor("x".repeat(CAPTION_MAX_CHARS + 50));
    let c = caption(&s).unwrap();
    assert_eq!(c.chars().count(), CAPTION_MAX_CHARS);
    assert!(c.ends_with('…'), "shortening is marked: {c:?}");
    // Text that already fits is shown verbatim, ellipsis-free.
    let fits = SourceContext::Cursor("y".repeat(CAPTION_MAX_CHARS));
    assert_eq!(
        caption(&fits).as_deref(),
        Some("y".repeat(CAPTION_MAX_CHARS)).as_deref()
    );
}

#[test]
fn cursor_caption_truncates_by_character_not_byte() {
    // Hovered text is arbitrary: CJK and emoji must bound by character count and
    // never slice mid-code-point (a byte slice here panics).
    let s = SourceContext::Cursor("秒🕰".repeat(200));
    let c = caption(&s).unwrap();
    assert_eq!(c.chars().count(), CAPTION_MAX_CHARS);
}

#[test]
fn clipboard_caption_renders_nothing() {
    // The privacy property one layer up from `source_caption`: the value the
    // header would draw is absent, so no clipboard content can reach the screen.
    assert_eq!(caption(&SourceContext::Clipboard), None);
}

#[test]
fn blank_cursor_text_captions_nothing() {
    // Preserves the header's old `!source.is_empty()` guard: an element with no
    // text (or only whitespace) draws no caption rather than an empty gap.
    assert_eq!(caption(&SourceContext::Cursor(String::new())), None);
    assert_eq!(caption(&SourceContext::Cursor(" \t\n ".to_owned())), None);
}
