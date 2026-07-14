//! Terminal colour capability ladder for the cal visual layer.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::cal_color::{detect, season_ink, ColorMode, EPOCH};

#[test]
fn detection_ladder() {
    // Explicit never/always.
    assert_eq!(
        detect("never", false, true, Some("truecolor"), None),
        ColorMode::Mono
    );
    assert_eq!(
        detect("always", false, false, Some("truecolor"), None),
        ColorMode::Truecolor
    );
    // auto: NO_COLOR or non-tty ⇒ mono.
    assert_eq!(
        detect("auto", true, true, Some("truecolor"), None),
        ColorMode::Mono
    );
    assert_eq!(
        detect("auto", false, false, Some("truecolor"), None),
        ColorMode::Mono
    );
    // auto on a tty picks capability.
    assert_eq!(
        detect("auto", false, true, Some("truecolor"), None),
        ColorMode::Truecolor
    );
    assert_eq!(
        detect("auto", false, true, None, Some("xterm-256color")),
        ColorMode::Ansi256
    );
    assert_eq!(
        detect("auto", false, true, None, Some("xterm")),
        ColorMode::Ansi16
    );
}

#[test]
fn paint_emits_sgr_only_when_colored() {
    // Mono is a no-op — no escape sequences (the pipe-safe invariant).
    assert_eq!(ColorMode::Mono.paint(EPOCH, "e"), "e");
    assert!(!ColorMode::Mono.reverse("*").contains('\x1b'));
    // Truecolor wraps in 38;2 SGR.
    let t = ColorMode::Truecolor.paint(EPOCH, "e");
    assert!(
        t.starts_with("\x1b[38;2;86;182;194m") && t.ends_with("\x1b[0m"),
        "{t:?}"
    );
    // 256 and 16 use their own codes.
    assert!(ColorMode::Ansi256.paint(EPOCH, "e").contains("38;5;80"));
    assert!(ColorMode::Ansi16.paint(EPOCH, "e").contains("[36m"));
}

#[test]
fn season_ink_maps_names() {
    assert_eq!(season_ink("summer").rgb, (0xf2, 0xc1, 0x4e));
    assert_eq!(season_ink("winter").rgb, (0x8a, 0xb6, 0xd6));
}
