//! Tests for decoding the clipboard — the path that works when hovering cannot
//! reach the value, above all inside a VM guest (the host accessibility tree stops
//! at the guest window, but the clipboard crosses that boundary; verified).
//!
//! The clipboard is injected behind a trait so these never touch the real system
//! clipboard: doing so would be flaky and would clobber the developer's clipboard.
#![allow(clippy::unwrap_used)]

use timeglyph::RenderZone;
use timeglyph_lens::clipboard::{
    caption, decode, read_decodable, source_caption, ClipboardRead, ClipboardUnavailable,
    SourceContext, SystemClipboard, CAPTION_MAX_CHARS,
};

/// A fake clipboard. Tests own the content, so nothing reads the real pasteboard.
struct Fake(Option<&'static str>);

impl ClipboardRead for Fake {
    fn text(&mut self) -> Option<String> {
        self.0.map(str::to_owned)
    }
}

/// A fake that counts reads, so a test can prove the decode is one-shot.
struct Counting {
    content: Option<&'static str>,
    reads: usize,
}

impl ClipboardRead for Counting {
    fn text(&mut self) -> Option<String> {
        self.reads += 1;
        self.content.map(str::to_owned)
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

#[test]
fn decoding_the_clipboard_yields_readings_and_a_textless_source() {
    let mut c = Fake(Some("  1721000000  \n"));
    let (source, hits) = decode(&mut c, 8, &RenderZone::Utc).unwrap();
    // The value is decoded exactly as hovered text is, padding and all.
    assert_eq!(hits.len(), 1, "one number in the clipboard, one card");
    assert!(
        !hits[0].readings.is_empty(),
        "a unix second count has readings"
    );
    // The source the overlay will hold retains no clipboard content, so the
    // header has nothing to draw — the privacy property at the seam that
    // actually feeds the UI.
    assert_eq!(source, SourceContext::Clipboard);
    assert_eq!(caption(&source), None);
}

#[test]
fn decoding_reads_the_clipboard_exactly_once() {
    // Pull-based by construction: one press, one read. Nothing polls, so there is
    // no continuous monitoring to disclose.
    let mut c = Counting {
        content: Some("1721000000"),
        reads: 0,
    };
    decode(&mut c, 8, &RenderZone::Utc).unwrap();
    assert_eq!(c.reads, 1);
}

#[test]
fn an_empty_clipboard_decodes_to_nothing() {
    // `None` tells the caller to keep whatever is on screen rather than blank it.
    assert!(decode(&mut Fake(None), 8, &RenderZone::Utc).is_none());
    assert!(decode(&mut Fake(Some("")), 8, &RenderZone::Utc).is_none());
    assert!(decode(&mut Fake(Some(" \t\n")), 8, &RenderZone::Utc).is_none());
}

#[test]
fn clipboard_text_with_no_timestamp_decodes_to_nothing() {
    // Same non-clobber rule the cursor path uses: text that decodes to nothing
    // must not wipe the reading the analyst is looking at.
    assert!(decode(&mut Fake(Some("no numbers here")), 8, &RenderZone::Utc).is_none());
}

#[test]
fn the_system_clipboard_is_a_readable_clipboard() {
    // Compile-time proof that the platform clipboard satisfies the trait the
    // decode path takes — without opening the real pasteboard, which would be
    // flaky and would clobber the developer's clipboard.
    fn assert_impl<C: ClipboardRead>() {}
    assert_impl::<SystemClipboard>();
}

#[test]
fn an_unavailable_clipboard_names_the_platform_reason() {
    // A host with no window server has no clipboard. That must read as a named
    // failure carrying the platform's own words — never as an empty clipboard,
    // which is indistinguishable from a genuinely empty one.
    let e = ClipboardUnavailable {
        reason: "the clipboard contents were not available".to_owned(),
    };
    let shown = e.to_string();
    assert!(
        shown.contains("the clipboard contents were not available"),
        "the reason is shown verbatim: {shown:?}"
    );
    assert!(
        shown.contains("clipboard"),
        "and it says what failed: {shown:?}"
    );
}
