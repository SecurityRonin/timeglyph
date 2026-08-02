//! The global hotkey that decodes the clipboard without reaching for the button.
//!
//! Everything decidable lives here and is unit-tested: which combination we bind,
//! how it reads on screen, which combinations we refuse, and what the user is told
//! when registration fails. Actually asking the platform to grab the key is a thin
//! shell in the binary, because it cannot be tested without a window server.
//!
//! # Why the default is built on T, not V
//!
//! A *global* grab takes the combination away from **every** application on the
//! machine, not just this one. That inverts the usual instinct: the memorable
//! choices are precisely the wrong ones. `Cmd+Shift+V` is "paste and match style" in
//! a great many applications and grabbing it globally would break that everywhere;
//! `Opt+Cmd+V` is Finder's paste-move; `Win+V` is Windows' clipboard history.
//!
//! The tempting escape is to keep V and stack modifiers until nothing collides — but
//! a four-modifier chord is uncontested precisely *because* nobody can press it, so
//! that trade buys safety with the one property a shortcut exists to provide. Safety
//! comes from the **key** instead: V is the most fought-over key on the keyboard, and
//! we never needed it. T — for *timestamp*, which is what the press decodes — is
//! quiet enough that two modifiers suffice, which keeps the chord one-handed.
//!
//! Two modifiers is also the floor: below it a global grab would steal a plain
//! keystroke from every application. So the default carries exactly two.
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
            // Every VoiceOver command is ⌃⌥ + something. Grabbing anything in that
            // namespace globally breaks screen-reader control — an unacceptable
            // trade for a tool that already leans on the accessibility API.
            if self.control && self.alt {
                return Some("⌃⌥ is VoiceOver's modifier — every VoiceOver command uses it");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if self.control && self.alt && self.key == '\u{7f}' {
                return Some("the system uses this combination");
            }
            // GNOME and most Linux desktops ship this as open-a-terminal.
            if self.control && self.alt && self.key == 'T' {
                return Some("Linux desktops use this to open a terminal");
            }
            // Every browser binds this to reopen-closed-tab; a global grab would take
            // it from all of them.
            if self.control && self.shift && self.key == 'T' {
                return Some("browsers use this to reopen a closed tab");
            }
        }
        None
    }

    /// The trigger key.
    #[must_use]
    pub const fn key_char(&self) -> char {
        self.key
    }

    /// This combination as `global-hotkey` expresses it, ready to hand to the OS.
    ///
    /// The seam worth getting right: if this disagrees with [`Self::display`] we bind
    /// one combination and advertise another, so the user presses what the tooltip
    /// says and nothing happens — indistinguishable from the silent shadowing we
    /// cannot detect.
    ///
    /// A key outside the mapped set becomes [`Code::Unidentified`], which the platform
    /// rejects at registration, surfacing as a named [`Registration::Failed`] rather
    /// than a binding that quietly never fires.
    #[must_use]
    pub fn to_global_hotkey(&self) -> global_hotkey::hotkey::HotKey {
        use global_hotkey::hotkey::{Code, HotKey, Modifiers};

        let mut mods = Modifiers::empty();
        if self.control {
            mods |= Modifiers::CONTROL;
        }
        if self.alt {
            mods |= Modifiers::ALT;
        }
        if self.shift {
            mods |= Modifiers::SHIFT;
        }
        if self.meta {
            mods |= Modifiers::META;
        }

        let code = match self.key.to_ascii_uppercase() {
            'A' => Code::KeyA,
            'B' => Code::KeyB,
            'C' => Code::KeyC,
            'D' => Code::KeyD,
            'E' => Code::KeyE,
            'F' => Code::KeyF,
            'G' => Code::KeyG,
            'H' => Code::KeyH,
            'I' => Code::KeyI,
            'J' => Code::KeyJ,
            'K' => Code::KeyK,
            'L' => Code::KeyL,
            'M' => Code::KeyM,
            'N' => Code::KeyN,
            'O' => Code::KeyO,
            'P' => Code::KeyP,
            'Q' => Code::KeyQ,
            'R' => Code::KeyR,
            'S' => Code::KeyS,
            'T' => Code::KeyT,
            'U' => Code::KeyU,
            'V' => Code::KeyV,
            'W' => Code::KeyW,
            'X' => Code::KeyX,
            'Y' => Code::KeyY,
            'Z' => Code::KeyZ,
            '0' => Code::Digit0,
            '1' => Code::Digit1,
            '2' => Code::Digit2,
            '3' => Code::Digit3,
            '4' => Code::Digit4,
            '5' => Code::Digit5,
            '6' => Code::Digit6,
            '7' => Code::Digit7,
            '8' => Code::Digit8,
            '9' => Code::Digit9,
            _ => Code::Unidentified,
        };
        HotKey::new(Some(mods), code)
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
/// **`T` for timestamp, and only two modifiers.** The obscurity that keeps a global
/// grab from stealing someone's shortcut comes from picking a quiet *key*, not from
/// piling on modifiers — a four-modifier chord is safe precisely because nobody can
/// comfortably press it, which makes it a bad shortcut.
///
/// `V` was the obvious choice and the wrong one: it is the paste key, so it is the
/// most contested key on the keyboard (`⌘V`, `⌘⇧V` paste-and-match, `⌥⌘V` Finder
/// paste-move, `Win+V` clipboard history). Competing there forces the modifier
/// stacking this constant now avoids.
///
/// Platform-conditional, because the quiet space differs:
/// - **macOS `⌃⌘T`** — avoids `⌃⌥`, which is VoiceOver's modifier for *every* one of
///   its commands, and `⌥⌘T`, which is Show Toolbar in Apple's own apps.
/// - **Windows / Linux `Alt+Shift+T`** — avoids `Ctrl+Alt+T` (GNOME's open-terminal)
///   and `Ctrl+Shift+T` (reopen-closed-tab in every browser).
#[cfg(target_os = "macos")]
pub const DEFAULT: HotkeySpec = HotkeySpec::new(true, false, false, true, 'T');
/// See the macOS variant above for the rationale.
#[cfg(not(target_os = "macos"))]
pub const DEFAULT: HotkeySpec = HotkeySpec::new(false, true, true, false, 'T');

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
    /// Never asked for — a headless run or the render gate.
    ///
    /// Distinct from [`Self::Unsupported`] on purpose: "we chose not to register" is
    /// not a failure and must not produce a user-facing notice, whereas "this
    /// platform cannot" is something the user needs told.
    NotInstalled,
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
            Self::Failed { .. } | Self::Unsupported(_) | Self::NotInstalled => None,
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
            // Silent by design: nothing was attempted, so there is nothing to report.
            Self::NotInstalled => None,
        }
    }
}

/// A live global hotkey, and the manager whose lifetime owns it.
///
/// The thin shell around the platform: everything decidable lives above and is
/// unit-tested, while this part cannot be exercised without a window server.
pub struct Hotkey {
    /// Dropping the manager unregisters the key, so it is held for the process
    /// lifetime rather than discarded after `install`.
    _manager: Option<global_hotkey::GlobalHotKeyManager>,
    registration: Registration,
}

impl Hotkey {
    /// Ask the platform for `spec`, reporting rather than panicking on refusal.
    ///
    /// Never fatal: on any failure the 🗐 button still works, so a taken combination
    /// costs convenience and not access.
    #[must_use]
    pub fn install(spec: HotkeySpec) -> Self {
        // Wayland has no protocol for an application to grab a global shortcut — it
        // belongs to the compositor by design. Under XWayland a grab can even appear
        // to succeed while the compositor routes the key natively and it never
        // fires, which is the undetectable failure; say so up front instead.
        #[cfg(all(unix, not(target_os = "macos")))]
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return Self {
                _manager: None,
                registration: Registration::Unsupported(
                    "global hotkeys are not available on Wayland — use the clipboard button",
                ),
            };
        }

        // Refuse what we already know is spoken for, rather than binding it and
        // letting it fail invisibly.
        if let Some(reason) = spec.reserved_reason() {
            return Self {
                _manager: None,
                registration: Registration::Failed {
                    spec,
                    reason: reason.to_owned(),
                },
            };
        }

        let manager = match global_hotkey::GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                return Self {
                    _manager: None,
                    registration: Registration::Failed {
                        spec,
                        reason: e.to_string(),
                    },
                }
            }
        };

        // The platform's own words are kept verbatim: "already registered by another
        // application" tells the user what to do, where "failed" does not.
        match manager.register(spec.to_global_hotkey()) {
            Ok(()) => {
                tracing::info!(binding = %spec.display(), "global hotkey registered");
                Self {
                    _manager: Some(manager),
                    registration: Registration::Active(spec),
                }
            }
            Err(e) => {
                tracing::warn!(binding = %spec.display(), error = %e, "global hotkey unavailable");
                Self {
                    _manager: None,
                    registration: Registration::Failed {
                        spec,
                        reason: e.to_string(),
                    },
                }
            }
        }
    }

    /// A hotkey that was never asked for.
    ///
    /// For tests and headless runs. The offscreen render gate must not grab a real
    /// system-wide key — that would reach outside the process and take a combination
    /// from the developer's machine for the duration of the test run, the same
    /// hermeticity concern that made the gate inject its clipboard and settings.
    #[must_use]
    pub fn none() -> Self {
        Self {
            _manager: None,
            registration: Registration::NotInstalled,
        }
    }

    /// What happened when we asked for the key.
    #[must_use]
    pub fn registration(&self) -> &Registration {
        &self.registration
    }

    /// Whether the hotkey was pressed since the last call, draining the queue.
    ///
    /// Polled once per frame from the render loop, mirroring how the cursor snapshot
    /// is consumed. Only `Pressed` counts, so one physical press is one decode rather
    /// than two.
    pub fn fired(&self) -> bool {
        let mut pressed = false;
        while let Ok(ev) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
            if ev.state == global_hotkey::HotKeyState::Pressed {
                pressed = true;
            }
        }
        pressed
    }
}
