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
//! Reading is **pull-based**: the overlay's 🗐 control reads the clipboard once, when
//! pressed. Nothing watches or polls it, so there is no background access to the
//! pasteboard and no surveillance state for the UI to disclose — the press is the
//! consent boundary.
//!
//! The clipboard is read through [`ClipboardRead`] so tests inject a fake and never
//! touch the real pasteboard (which would be flaky and would clobber the developer's
//! clipboard).

use std::fmt;

use timeglyph::RenderZone;

use crate::scan::{self, NumberReadings};

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
/// sensitive than a hovered element, so the variant is made incapable of holding text —
/// the type enforces it, rather than a conditional one missed branch could defeat.
///
/// Scope, precisely: this protects the **caption**, not the reading. A value the engine
/// recognises is still printed verbatim as the card's subject — that is the point of
/// pressing the control — so a copied bearer token or hex recovery code that decodes
/// *will* appear on screen. What this prevents is the surrounding raw context (and the
/// non-decoding remainder of the clipboard) being drawn or logged as well.
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

/// Longest source caption the overlay's header draws, in characters. The caption is
/// context, not the subject, so it stays on one line and yields the panel to the
/// readings.
pub const CAPTION_MAX_CHARS: usize = 120;

/// The source caption ready to draw: whitespace collapsed to single spaces and
/// bounded to [`CAPTION_MAX_CHARS`] characters, or `None` when there is nothing to
/// draw — the clipboard (which retains no text) or an element whose text is blank.
///
/// Bounding is by character, never by byte: hovered text is arbitrary, and a byte
/// slice through CJK or emoji panics.
#[must_use]
pub fn caption(source: &SourceContext) -> Option<String> {
    let collapsed = source_caption(source)?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.is_empty() {
        return None;
    }
    Some(crate::text::ellipsize(&collapsed, CAPTION_MAX_CHARS))
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

/// Read the clipboard **once** and decode it, returning the source to display
/// alongside the readings — or `None` when the clipboard holds nothing, or nothing
/// that decodes.
///
/// `None` means *leave the display alone*: the same rule the cursor path follows, so
/// pressing 🗐 with an unrelated value copied cannot wipe the reading being read.
///
/// The source and the readings are returned together, as one value, so a caller
/// cannot show clipboard readings under a stale cursor caption.
#[must_use]
pub fn decode(
    clipboard: &mut dyn ClipboardRead,
    max_per_number: usize,
    zone: &RenderZone,
) -> Option<(SourceContext, Vec<NumberReadings>)> {
    let text = read_decodable(clipboard)?;
    let hits = scan::inspect_text(&text, max_per_number, zone);
    if hits.is_empty() {
        return None;
    }
    // `text` dies here: only the readings and a textless source leave this function.
    Some((SourceContext::Clipboard, hits))
}

/// The platform clipboard could not be opened — a host with no window server or
/// pasteboard has no clipboard at all.
///
/// A distinct, named failure rather than an empty read: "no clipboard here" and "the
/// clipboard is empty" demand different responses from the UI, and collapsing them
/// would hide the former behind the latter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardUnavailable {
    /// The platform's own reason, verbatim, so the diagnostic names what failed
    /// instead of asserting a generic "unavailable".
    pub reason: String,
}

impl fmt::Display for ClipboardUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the platform clipboard is unavailable: {}", self.reason)
    }
}

impl std::error::Error for ClipboardUnavailable {}

/// The platform clipboard (macOS pasteboard, Windows clipboard, X11/Wayland
/// selection), via `arboard`.
pub struct SystemClipboard(arboard::Clipboard);

impl SystemClipboard {
    /// Open the platform clipboard.
    ///
    /// # Errors
    /// [`ClipboardUnavailable`] when the host has no clipboard to open (headless, no
    /// display), carrying the platform's reason.
    pub fn new() -> Result<Self, ClipboardUnavailable> {
        arboard::Clipboard::new().map(Self).map_err(|e| {
            let reason = e.to_string();
            tracing::warn!(reason = %reason, "the platform clipboard could not be opened");
            ClipboardUnavailable { reason }
        })
    }
}

impl ClipboardRead for SystemClipboard {
    fn text(&mut self) -> Option<String> {
        match self.0.get_text() {
            Ok(text) => Some(text),
            // The one genuine "nothing to decode": the pasteboard is empty, or holds
            // something that is not text (an image, a file promise).
            Err(arboard::Error::ContentNotAvailable) => None,
            // Anything else is a real failure of the read, not an empty clipboard —
            // name it and the reason. Never the content: a log line is as readable
            // as the caption this type exists to prevent.
            Err(e) => {
                tracing::warn!(error = %e, "reading the clipboard failed");
                None
            }
        }
    }
}
