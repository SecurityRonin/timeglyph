//! Decoding the **clipboard**, for values the cursor picker cannot reach.
//!
//! The picker reads the UI element under the pointer through the platform
//! accessibility layer, and that tree **stops at a VM guest window**: the host sees
//! one opaque framebuffer, so hovering a timestamp inside a guest yields nothing.
//! The clipboard does cross that boundary (verified guest→host on a real VM), so
//! copying the value and decoding the clipboard is the way in.
//!
//! Scope: this helps when the guest runs **windowed**. It cannot help a *full-screen*
//! guest, because macOS excludes the overlay from a full-screen Space entirely — see
//! `docs/lens.md`; for that case use the CLI (`pbpaste | timeglyph scan`).
//!
//! The clipboard is read through [`ClipboardRead`] so tests inject a fake and never
//! touch the real pasteboard (which would be flaky and would clobber the developer's
//! clipboard).

/// A readable clipboard. Implemented by the platform clipboard in the binary, and by
/// a fake in tests.
pub trait ClipboardRead {
    /// The clipboard's current text, or `None` when it is empty or holds non-text.
    fn text(&mut self) -> Option<String>;
}

/// Where the text being decoded came from.
///
/// `Clipboard` deliberately carries **no text**. The overlay renders the source as a
/// caption in an always-on-top window, and clipboard content is categorically more
/// sensitive than a hovered element — a copied password, recovery code or OTP would
/// otherwise be displayed. Making the variant incapable of holding text enforces that
/// with the type system instead of a conditional that one missed branch could defeat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceContext {
    /// Text read from the UI element under the cursor, safe to caption.
    Cursor(String),
    /// The clipboard — no content retained for display.
    Clipboard,
}

/// The de-emphasised source caption, or `None` when there is nothing displayable.
#[must_use]
pub fn source_caption(source: &SourceContext) -> Option<&str> {
    match source {
        SourceContext::Cursor(text) => Some(text),
        // No text exists to return — see the type's docs.
        SourceContext::Clipboard => None,
    }
}

/// Read the clipboard and return trimmed text worth decoding, or `None` when it is
/// empty, non-text, or blank. Copying from a log or table usually drags padding
/// along, so trimming is part of the contract rather than the caller's problem.
pub fn read_decodable(clipboard: &mut dyn ClipboardRead) -> Option<String> {
    let text = clipboard.text()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}
