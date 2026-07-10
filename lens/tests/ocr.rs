//! The OCR-fallback *decision* layer: prefer the accessibility API's exact text,
//! fall back to reading pixels only when AX yields nothing. This is the testable
//! core (Humble Object) — the pixel→text FFI itself is a platform backend that
//! implements [`RegionOcr`] and is exercised on-device, not here.

use std::cell::Cell;
use timeglyph_lens::ocr::{resolve_text, RegionOcr};

/// A backend that records whether it was consulted, so we can prove OCR is only
/// invoked as a fallback (never when AX already has text).
struct SpyOcr {
    reply: Option<&'static str>,
    called: Cell<bool>,
}

impl RegionOcr for SpyOcr {
    fn text_near(&self, _x: f64, _y: f64) -> Option<String> {
        self.called.set(true);
        self.reply.map(str::to_string)
    }
}

#[test]
fn accessibility_text_wins_and_ocr_is_not_consulted() {
    let spy = SpyOcr {
        reply: Some("should-not-be-used"),
        called: Cell::new(false),
    };
    let got = resolve_text(Some("1577836800".into()), || spy.text_near(0.0, 0.0));
    assert_eq!(got.as_deref(), Some("1577836800"));
    assert!(
        !spy.called.get(),
        "OCR must not run when the accessibility API already returned text"
    );
}

#[test]
fn blank_accessibility_text_falls_through_to_ocr() {
    // A canvas app can return an empty/whitespace string rather than None; that
    // is still "no text", so OCR should be consulted.
    let spy = SpyOcr {
        reply: Some("1746371930"),
        called: Cell::new(false),
    };
    let got = resolve_text(Some("   ".into()), || spy.text_near(10.0, 20.0));
    assert_eq!(got.as_deref(), Some("1746371930"));
    assert!(spy.called.get(), "blank AX text must fall through to OCR");
}

#[test]
fn none_accessibility_falls_through_to_ocr() {
    let spy = SpyOcr {
        reply: Some("133908455300649390"),
        called: Cell::new(false),
    };
    let got = resolve_text(None, || spy.text_near(0.0, 0.0));
    assert_eq!(got.as_deref(), Some("133908455300649390"));
    assert!(spy.called.get());
}

#[test]
fn blank_ocr_result_is_treated_as_no_text() {
    // OCR that finds only whitespace/nothing must yield None, not a blank string
    // that would later scan to zero readings and look like a bug.
    let spy = SpyOcr {
        reply: Some("  \n "),
        called: Cell::new(false),
    };
    assert_eq!(resolve_text(None, || spy.text_near(0.0, 0.0)), None);
}

#[test]
fn no_backend_preserves_the_accessibility_only_behaviour() {
    // The default (no OCR backend) leaves behaviour exactly as before: AX text
    // passes through, and AX-None stays None.
    assert_eq!(
        resolve_text(Some("42".into()), || None).as_deref(),
        Some("42")
    );
    assert_eq!(resolve_text(None, || None), None);
}
