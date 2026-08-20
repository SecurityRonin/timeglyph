//! Terminal colour for the `cal` visual layer — a capability ladder
//! (truecolor → 256 → 16 → monochrome) applied as ANSI SGR escapes. Colour is
//! garnish: every marker/glyph is already a distinct character, so the monochrome
//! output loses colour but never information. Detection lives in the shell (env is
//! passed in, not read here) so the renderers stay pure and testable, and ANSI is
//! never emitted into the machine (`--json`/`--tsv`) views.

/// The terminal's colour capability, resolved once by [`detect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// 24-bit truecolor (`\x1b[38;2;r;g;bm`).
    Truecolor,
    /// 256-colour (`\x1b[38;5;Nm`).
    Ansi256,
    /// 16-colour (`\x1b[3Nm`).
    Ansi16,
    /// No colour — plain text.
    Mono,
}

/// A palette entry: the RGB (truecolor), the 256-colour index, and the 16-colour
/// SGR digit (30–37), so one colour degrades cleanly across the ladder.
#[derive(Debug, Clone, Copy)]
pub struct Ink {
    pub rgb: (u8, u8, u8),
    pub c256: u8,
    pub c16: u8,
}

impl ColorMode {
    /// Wrap `text` in the SGR for `ink` at this capability (no-op under [`ColorMode::Mono`]).
    #[must_use]
    pub fn paint(self, ink: Ink, text: &str) -> String {
        let (r, g, b) = ink.rgb;
        match self {
            ColorMode::Truecolor => format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[0m"),
            ColorMode::Ansi256 => format!("\x1b[38;5;{}m{text}\x1b[0m", ink.c256),
            ColorMode::Ansi16 => format!("\x1b[{}m{text}\x1b[0m", ink.c16),
            ColorMode::Mono => text.to_string(),
        }
    }

    /// Wrap `text` in reverse video (used for `today`); no-op under [`ColorMode::Mono`].
    #[must_use]
    pub fn reverse(self, text: &str) -> String {
        if self == ColorMode::Mono {
            text.to_string()
        } else {
            format!("\x1b[7m{text}\x1b[0m")
        }
    }
}

/// Resolve the colour mode from the `--color` argument and the environment. Pure:
/// the shell reads the env and TTY state and passes them in.
///
/// `auto` (the default) honours `NO_COLOR` (any value ⇒ off) and a non-TTY stdout,
/// then picks truecolor / 256 / 16 from `$COLORTERM` / `$TERM`.
#[must_use]
pub fn detect(
    color_arg: &str,
    no_color: bool,
    is_tty: bool,
    colorterm: Option<&str>,
    term: Option<&str>,
) -> ColorMode {
    match color_arg {
        "never" => return ColorMode::Mono,
        "always" => {} // force colour on; capability picked below
        _ => {
            // auto
            if no_color || !is_tty {
                return ColorMode::Mono;
            }
        }
    }
    let ct = colorterm.unwrap_or("");
    if ct.contains("truecolor") || ct.contains("24bit") {
        ColorMode::Truecolor
    } else if term.unwrap_or("").contains("256") {
        ColorMode::Ansi256
    } else {
        ColorMode::Ansi16
    }
}

// --- Palette (the `cal` colour scheme) ----------------------------------------

/// `today` marker uses reverse video, not a colour.
/// DST spring-forward gap (`^`).
pub const GAP: Ink = Ink {
    rgb: (0xff, 0x5f, 0x56),
    c256: 203,
    c16: 31,
}; // red
/// DST fall-back fold (`v`).
pub const FOLD: Ink = Ink {
    rgb: (0xf5, 0xc5, 0x18),
    c256: 220,
    c16: 33,
}; // yellow
/// Leap-second day (`+`).
pub const LEAP: Ink = Ink {
    rgb: (0xc6, 0x78, 0xdd),
    c256: 170,
    c16: 35,
}; // magenta
/// Format epoch day (`e`).
pub const EPOCH: Ink = Ink {
    rgb: (0x56, 0xb6, 0xc2),
    c256: 80,
    c16: 36,
}; // cyan
/// Fixed-width rollover (`~`).
pub const ROLLOVER: Ink = Ink {
    rgb: (0x61, 0xaf, 0xef),
    c256: 75,
    c16: 34,
}; // blue
/// The moon's lit `@`.
pub const MOON_LIT: Ink = Ink {
    rgb: (0xf5, 0xf3, 0xce),
    c256: 230,
    c16: 33,
}; // cream
/// The moon's dark `.`.
pub const MOON_DARK: Ink = Ink {
    rgb: (0x3a, 0x3a, 0x4a),
    c256: 237,
    c16: 30,
};
