//! The global hotkey's *decidable* parts: which combination we bind by default, how
//! it reads on screen, which combinations we refuse, and what the user is told when
//! registration fails.
//!
//! None of this touches the platform. Registering is a thin shell around
//! `global-hotkey`; everything a test can pin down lives here.
//!
//! The governing fact, which drives most of these cases: a *global* grab takes the
//! combination away from every application on the machine. So the memorable choices
//! are exactly the wrong ones — binding Cmd+Shift+V would break "paste and match
//! style" everywhere, and Opt+Cmd+V would break Finder's paste-move.
#![allow(clippy::unwrap_used)]

use timeglyph_lens::hotkey::{HotkeySpec, Registration, DEFAULT};

#[test]
fn the_default_binding_is_quiet_without_being_unpressable() {
    // Two properties in tension, and the earlier version got the trade wrong by
    // demanding lots of modifiers. A four-modifier chord IS collision-proof — because
    // nobody can comfortably press it, which makes it a bad shortcut.
    //
    // Safety should come from choosing a quiet KEY instead. So: at least two
    // modifiers (below that, a global grab steals a plain key from every app), but
    // not more than two, because it has to stay pressable one-handed.
    let n = DEFAULT.modifier_count();
    assert!(
        (2..=2).contains(&n),
        "default should carry exactly 2 modifiers, has {n}: {}",
        DEFAULT.display()
    );

    // Not the paste key. V is the most contested key on the keyboard (Cmd+V,
    // Cmd+Shift+V paste-and-match, Opt+Cmd+V Finder paste-move, Win+V clipboard
    // history) — competing there is what forced the modifier stacking above.
    assert_ne!(
        DEFAULT.key_char(),
        'V',
        "the default must not build on the contested paste key"
    );

    // And it must not be one we ourselves refuse.
    assert_eq!(
        DEFAULT.reserved_reason(),
        None,
        "the default must not be on our own deny-list: {}",
        DEFAULT.display()
    );
}

#[test]
fn the_combinations_we_learned_to_avoid_stay_refused() {
    // Each of these was a candidate default until it turned out to be taken. Pinning
    // them stops a future edit quietly reintroducing one.
    #[cfg(target_os = "macos")]
    {
        // Every VoiceOver command is Ctrl+Opt + something.
        let vo = HotkeySpec::new(true, true, false, false, 'T');
        assert!(
            vo.reserved_reason()
                .is_some_and(|r| r.contains("VoiceOver")),
            "Ctrl+Opt must stay refused — it is the screen reader's namespace"
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        let term = HotkeySpec::new(true, true, false, false, 'T');
        assert!(
            term.reserved_reason()
                .is_some_and(|r| r.contains("terminal")),
            "Ctrl+Alt+T must stay refused — Linux desktops open a terminal with it"
        );
        let tab = HotkeySpec::new(true, false, true, false, 'T');
        assert!(
            tab.reserved_reason().is_some_and(|r| r.contains("reopen")),
            "Ctrl+Shift+T must stay refused — browsers reopen a closed tab with it"
        );
    }
}

#[test]
fn a_binding_renders_for_the_platform_it_is_shown_on() {
    let shown = DEFAULT.display();
    assert!(!shown.is_empty(), "a binding must be displayable");
    // The user has to be able to read it off the tooltip and press it. Whatever the
    // platform convention, the trigger key has to appear.
    assert!(
        shown.to_uppercase().contains('V'),
        "the trigger key must be visible in {shown:?}"
    );
}

#[test]
fn combinations_the_operating_system_owns_are_refused_with_a_reason() {
    // Binding these would either fail or, worse, appear to succeed and never fire.
    // Refusing up front with a stated reason beats silent shadowing.
    for spec in HotkeySpec::examples_reserved() {
        let why = spec.reserved_reason();
        assert!(
            why.is_some(),
            "{} is owned by the OS and must be refused",
            spec.display()
        );
        assert!(
            !why.unwrap().is_empty(),
            "a refusal must say why, not just refuse"
        );
    }
}

#[test]
fn a_failed_registration_names_the_combination_and_the_reason() {
    // Fail loud: "the hotkey didn't work" is useless. The user needs to know which
    // combination and what went wrong, so they can rebind.
    let r = Registration::Failed {
        spec: DEFAULT,
        reason: "already registered by another application".to_owned(),
    };
    let note = r.notice().expect("a failure must be reported to the user");
    assert!(
        note.contains(&DEFAULT.display()),
        "the notice must name the combination: {note:?}"
    );
    assert!(
        note.contains("already registered"),
        "the notice must carry the platform's own reason: {note:?}"
    );
}

#[test]
fn an_unsupported_platform_says_so_rather_than_looking_broken() {
    // Wayland has no global-hotkey support. Silence there would read as a bug.
    let r = Registration::Unsupported("global hotkeys are not available on Wayland");
    let note = r
        .notice()
        .expect("an unsupported platform must be reported");
    assert!(note.to_lowercase().contains("wayland"), "{note:?}");
}

#[test]
fn a_working_registration_reports_nothing_and_exposes_its_binding() {
    let r = Registration::Active(DEFAULT);
    assert_eq!(
        r.notice(),
        None,
        "a hotkey that works needs no explanation; it speaks by firing"
    );
    assert_eq!(
        r.active_binding().map(|s| s.display()),
        Some(DEFAULT.display()),
        "the active binding must be readable, for the tooltip"
    );
}

#[test]
fn a_failed_registration_exposes_no_active_binding() {
    // The tooltip must not advertise a shortcut that will never fire.
    let r = Registration::Failed {
        spec: DEFAULT,
        reason: "taken".to_owned(),
    };
    assert!(r.active_binding().is_none());
    assert!(Registration::Unsupported("nope").active_binding().is_none());
}

#[test]
fn a_spec_converts_to_the_platform_hotkey_it_describes() {
    // The seam between our platform-neutral spec and what global-hotkey hands the
    // OS. Getting this wrong binds a DIFFERENT combination than the one shown in the
    // tooltip — the user presses what the UI told them and nothing happens, which
    // looks exactly like the silent-shadowing failure we cannot otherwise detect.
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};

    // Built independently of the implementation, so this compares against the intent
    // rather than against itself. Platform-conditional because the quiet part of the
    // keyboard differs: macOS must dodge VoiceOver's Ctrl+Opt, and Linux must dodge
    // Ctrl+Alt+T (open terminal).
    #[cfg(target_os = "macos")]
    let expected = HotKey::new(Some(Modifiers::CONTROL | Modifiers::META), Code::KeyT);
    #[cfg(not(target_os = "macos"))]
    let expected = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyT);

    assert_eq!(
        DEFAULT.to_global_hotkey(),
        expected,
        "the default must bind exactly what display() advertises: {}",
        DEFAULT.display()
    );
}

#[test]
fn shift_is_carried_into_the_platform_hotkey() {
    // A modifier dropped in conversion is invisible until someone presses the key.
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};

    let spec = HotkeySpec::new(false, false, true, true, 'V'); // Shift+Meta+V
    assert_eq!(
        spec.to_global_hotkey(),
        HotKey::new(Some(Modifiers::SHIFT | Modifiers::META), Code::KeyV)
    );
}
