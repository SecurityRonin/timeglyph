//! OCR fallback for pixel-only surfaces — RDP/Citrix sessions, VM consoles,
//! canvas-drawn apps (Electron `<canvas>`, some Java/Qt), and PDF/image viewers —
//! where the accessibility API exposes no selectable text and the picker returns
//! `None`. A [`RegionOcr`] backend reads text from the pixels around the cursor.
//!
//! [`resolve_text`] is the whole *decision*: prefer the accessibility API's exact
//! text and consult OCR only when AX yields nothing, so the fast, character-exact
//! path stays primary and OCR is a genuine fallback (slower, approximate) rather
//! than a competitor. Keeping the decision here — pure, injectable, unit-tested —
//! leaves each platform backend as a thin pixel→text shell (Humble Object): the
//! orchestration is proven in CI; only the FFI needs on-device verification.

/// Recognise text from the screen region around a screen point `(x, y)`.
///
/// Implementations capture a small region centred on the cursor and run the
/// platform OCR engine (macOS Vision, Windows `Windows.Media.Ocr`, or a
/// cross-platform engine on Linux). `None` means nothing legible was found or
/// the platform has no backend wired up.
pub trait RegionOcr {
    /// The recognised text near the screen point, or `None`.
    fn text_near(&self, x: f64, y: f64) -> Option<String>;
}

/// A no-op backend: the default when no OCR engine is wired in, preserving the
/// accessibility-only behaviour exactly (it always declines).
///
/// A native backend drops in here without touching [`resolve_text`]. The macOS
/// recipe: capture the region around the cursor with
/// `CGDisplayCreateImageForRect` (already available via the `core-graphics`
/// dep), then run `VNRecognizeTextRequest` over it via `objc2-vision`; join the
/// recognised lines and return them. It needs the Screen Recording permission
/// and on-device verification, so it lands behind an `ocr` feature rather than
/// on by default. Windows uses `Windows.Media.Ocr`; Linux, a bundled engine.
pub struct NoOcr;

impl RegionOcr for NoOcr {
    fn text_near(&self, _x: f64, _y: f64) -> Option<String> {
        None
    }
}

/// Prefer the accessibility text; fall back to OCR only when AX gave nothing.
///
/// A blank or whitespace-only AX result counts as "no text" (a canvas app can
/// return an empty string rather than `None`), so it too falls through to OCR.
/// A blank OCR result is likewise squashed to `None` — an empty string would
/// otherwise scan to zero readings and read like a bug rather than "nothing
/// here". The OCR closure is only evaluated when needed, so the capture+recognise
/// cost is never paid on the common AX-hit path.
pub fn resolve_text<F>(ax: Option<String>, ocr: F) -> Option<String>
where
    F: FnOnce() -> Option<String>,
{
    match ax {
        Some(s) if !s.trim().is_empty() => Some(s),
        _ => ocr().filter(|s| !s.trim().is_empty()),
    }
}
