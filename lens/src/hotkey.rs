//! The global hotkey that decodes the clipboard without reaching for the button.
//!
//! Everything decidable lives here and is unit-tested: which combination we bind,
//! how it reads on screen, which combinations we refuse, and what the user is told
//! when registration fails. Actually asking the platform to grab the key is a thin
//! shell in the binary, because it cannot be tested without a window server.
//!
//! # Why the default looks unmemorable
//!
//! A *global* grab takes the combination away from **every** application on the
//! machine, not just this one. That inverts the usual instinct: the memorable
//! choices are precisely the wrong ones. `Cmd+Shift+V` is "paste and match style" in
//! a great many applications and grabbing it globally would break that everywhere;
//! `Opt+Cmd+V` is Finder's paste-move. So the default deliberately carries enough
//! modifiers to be uncontested, and the binding is shown in the button's tooltip so
//! it stays discoverable despite being obscure.
//!
//! # The three ways a hotkey fails
//!
//! 1. **Registration errors** — Windows returns one when the combination is taken,
//!    X11 raises `BadAccess`. Detectable, so [`Registration::Failed`] names the
//!    combination *and* the platform's own reason and the app keeps running.
//! 2. **Silent shadowing** — on macOS registration can succeed while a system
//!    shortcut still consumes the key, so it simply never fires. Undetectable from
//!    here. [`HotkeySpec::reserved_reason`] refuses the combinations we know the OS
//!    owns; the rest is why the UI needs a "test it" affordance.
//! 3. **No support at all** — Wayland has none. [`Registration::Unsupported`] says
//!    so, because silence there reads as a bug.
//!
//! In every failure the 🗐 button still works, so this degrades rather than breaks.

use std::fmt::Write as _;

/// A modifier key, platform-neutral. Named for what it *is*, so the display layer
/// can render Mac glyphs or Windows/Linux words without the core knowing which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    /// `Control` everywhere.
    Control,
    /// `Option` on macOS, `Alt` elsewhere.
    Alt,
    /// `Shift` everywhere.
    Shift,
    /// `Command` on macOS, `Super`/`Win` elsewhere.
    Meta,
}

impl Modifier {
    /// How this modifier is written on the current platform.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            match self {
                Self::Control => "⌃",
                Self::Alt => "⌥",
                Self::Shift => "⇧",
                Self::Meta => "⌘",
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            match self {
                Self::Control => "Ctrl+",
                Self::Alt => "Alt+",
                Self::Shift => "Shift+",
                Self::Meta => "Super+",
            }
        }
    }
}

/// A key combination: some modifiers plus one trigger key.
///
/// Only the shapes we actually bind are representable — a single ASCII trigger key
/// and a fixed modifier set — which keeps the deny-list and the display honest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeySpec {
    control: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    /// The trigger key, uppercase ASCII.
    key: char,
}

impl HotkeySpec {
    /// Build a combination.
    #[must_use]
    pub const fn new(control: bool, alt: bool, shift: bool, meta: bool, key: char) -> Self {
        Self {
            control,
            alt,
            shift,
            meta,
            key,
        }
    }

    /// How many modifiers this combination carries.
    ///
    /// A low count is the tell that a binding will collide with an application
    /// shortcut and steal it machine-wide.
    #[must_use]
    pub const fn modifier_count(&self) -> usize {
        self.control as usize + self.alt as usize + self.shift as usize + self.meta as usize
    }

    /// The combination as the user must press it, in platform convention.
    #[must_use]
    pub fn display(&self) -> String {
        let mut s = String::new();
        for (on, m) in [
            (self.control, Modifier::Control),
            (self.alt, Modifier::Alt),
            (self.shift, Modifier::Shift),
            (self.meta, Modifier::Meta),
        ] {
            if on {
                s.push_str(m.glyph());
            }
        }
        let _ = write!(s, "{}", self.key);
        s
    }

    /// Why this combination is refused, or `None` if it is bindable.
    ///
    /// Refusing up front beats the alternative: these tend to register "successfully"
    /// and then never fire, because the OS consumes the key first — a failure we
    /// cannot detect and the user cannot diagnose.
    #[must_use]
    pub fn reserved_reason(&self) -> Option<&'static str> {
        // Bare or single-modifier combinations are not OS-owned as such, but binding
        // one globally steals a key from every application. Treated the same way.
        if self.modifier_count() < 2 {
            return Some(
                "too few modifiers — a global grab would take this key from every application",
            );
        }
        #[cfg(target_os = "macos")]
        {
            // ⌘Space Spotlight, ⌘Tab app switcher, ⌘⇧3/4/5 screenshots.
            if self.meta && !self.control && !self.alt && self.key == ' ' {
                return Some("macOS uses this for Spotlight");
            }
            if self.meta && self.key == '\t' {
                return Some("macOS uses this for the application switcher");
            }
            if self.meta && self.shift && matches!(self.key, '3' | '4' | '5') {
                return Some("macOS uses this for screenshots");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if self.control && self.alt && self.key == '\u{7f}' {
                return Some("the system uses this combination");
            }
        }
        None
    }

    /// Combinations known to be OS-owned, for tests to assert we refuse them.
    #[must_use]
    pub fn examples_reserved() -> Vec<Self> {
        let mut v = vec![
            // Too few modifiers: would steal a plain key machine-wide.
            Self::new(false, false, false, false, 'V'),
            Self::new(false, false, false, true, 'V'),
        ];
        #[cfg(target_os = "macos")]
        {
            v.push(Self::new(false, false, false, true, ' ')); // ⌘Space
            v.push(Self::new(false, false, true, true, '4')); // ⌘⇧4
        }
        #[cfg(not(target_os = "macos"))]
        {
            v.push(Self::new(true, true, false, false, '\u{7f}'));
        }
        v
    }
}

/// The binding we register unless the user chooses another.
///
/// Four modifiers, and `V` for the value being pasted. Obscure on purpose — see the
/// module docs on why a memorable default would be actively harmful.
pub const DEFAULT: HotkeySpec = HotkeySpec::new(true, true, false, true, 'V');

/// What happened when we asked the platform for the key.
#[derive(Debug)]
pub enum Registration {
    /// The key is ours and will fire.
    Active(HotkeySpec),
    /// The platform refused, and said why.
    Failed {
        /// The combination we asked for.
        spec: HotkeySpec,
        /// The platform's own words, kept verbatim so the diagnostic names the real
        /// cause rather than a guess.
        reason: String,
    },
    /// This platform has no global hotkeys at all.
    Unsupported(&'static str),
}

impl Registration {
    /// The combination the user can actually press, or `None` when there isn't one.
    ///
    /// Deliberately `None` for both failure cases, so a tooltip cannot advertise a
    /// shortcut that will never fire.
    #[must_use]
    pub fn active_binding(&self) -> Option<HotkeySpec> {
        match self {
            Self::Active(s) => Some(*s),
            Self::Failed { .. } | Self::Unsupported(_) => None,
        }
    }

    /// What to tell the user, or `None` when the hotkey works and needs no
    /// explanation — it speaks by firing.
    ///
    /// A failure names the combination *and* the platform's reason: "the hotkey
    /// didn't work" gives the user nothing to act on, whereas naming both tells them
    /// what to rebind and why.
    #[must_use]
    pub fn notice(&self) -> Option<String> {
        match self {
            Self::Active(_) => None,
            Self::Failed { spec, reason } => {
                Some(format!("hotkey {} unavailable — {reason}", spec.display()))
            }
            Self::Unsupported(why) => Some((*why).to_owned()),
        }
    }
}
