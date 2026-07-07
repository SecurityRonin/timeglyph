//! Both overlay palettes must stay legible: every text tone clears WCAG AA
//! contrast against the panel background. This guards the light palette (whose
//! amber accent must be darkened to clear AA on a light ground) as much as the
//! dark one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use eframe::egui::Color32;
use timeglyph_lens::theme::Theme;

/// WCAG relative luminance of an sRGB colour.
fn luminance(c: Color32) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio between two colours (1.0 .. 21.0).
fn contrast(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn text_tones_clear_wcag_aa_on_panel() {
    for theme in [Theme::Dark, Theme::Light] {
        let p = theme.palette();
        // Primary text tones must clear AA for normal text (4.5:1).
        for (tone, c) in [("ink", p.ink), ("amber", p.amber), ("mute", p.mute)] {
            let r = contrast(c, p.bg_deep);
            assert!(
                r >= 4.5,
                "{theme:?} {tone}: contrast {r:.2} < 4.5 (WCAG AA)"
            );
        }
        // Captions (faint) are secondary — AA for large/UI text (3:1) is enough.
        let r = contrast(p.faint, p.bg_deep);
        assert!(r >= 3.0, "{theme:?} faint: contrast {r:.2} < 3.0");
    }
}

#[test]
fn confidence_dots_are_visible_on_panel() {
    // The traffic-light confidence dots are indicators, not text, but must still
    // read against the panel on both themes (AA non-text / UI ≥ 3:1) — the light
    // palette especially, where a bright amber/green would wash out.
    for theme in [Theme::Dark, Theme::Light] {
        let p = theme.palette();
        for (name, c) in [
            ("conf_high", p.conf_high),
            ("conf_mid", p.conf_mid),
            ("conf_low", p.conf_low),
        ] {
            let r = contrast(c, p.bg_deep);
            assert!(r >= 3.0, "{theme:?} {name}: contrast {r:.2} < 3.0");
        }
    }
}

#[test]
fn effective_theme_follows_system_when_no_explicit_preference() {
    use timeglyph_lens::theme::effective_theme;
    // No saved preference (None) → follow the OS setting.
    assert_eq!(effective_theme(None, Some(Theme::Light)), Theme::Light);
    assert_eq!(effective_theme(None, Some(Theme::Dark)), Theme::Dark);
    // An explicit choice always wins over the system.
    assert_eq!(
        effective_theme(Some(Theme::Light), Some(Theme::Dark)),
        Theme::Light
    );
    assert_eq!(
        effective_theme(Some(Theme::Dark), Some(Theme::Light)),
        Theme::Dark
    );
    // Neither known → safe fallback to Dark.
    assert_eq!(effective_theme(None, None), Theme::Dark);
}
