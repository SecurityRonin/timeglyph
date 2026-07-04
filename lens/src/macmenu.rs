//! The native macOS application menu with standard `About timeglyph-lens` and
//! `Settings…` (⌘,) items, following normal Mac convention. The menu bar shows
//! only while timeglyph-lens is frontmost, so it complements — does not replace —
//! the in-window ⚙ button and the clickable corner logo (both reachable while
//! the tool is unfocused).
//!
//! No-op on other platforms (egui draws its own chrome there).

/// Which menu item(s) fired since the last poll.
#[derive(Default, Clone, Copy)]
pub struct Selected {
    pub settings: bool,
    pub about: bool,
}

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::OnceLock;

    use muda::accelerator::{Accelerator, Code, Modifiers};
    use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

    use super::Selected;

    static SETTINGS_ID: OnceLock<MenuId> = OnceLock::new();
    static ABOUT_ID: OnceLock<MenuId> = OnceLock::new();

    /// Build the app menu (About · Settings… ⌘, · Hide · Quit) and set it as the
    /// `NSApplication` main menu. Call once, on the main thread, after init. The
    /// About item is a plain item we handle ourselves (a custom dialog), not the
    /// system about panel.
    pub fn install() {
        let menu = Menu::new();
        let app = Submenu::new("TimeGlyph Lens", true);
        let about = MenuItem::new("About TimeGlyph Lens", true, None);
        let settings = MenuItem::new(
            "Settings…",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)),
        );
        let _ = app.append_items(&[
            &about,
            &PredefinedMenuItem::separator(),
            &settings,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::quit(None),
        ]);
        let _ = menu.append(&app);
        menu.init_for_nsapp();
        let _ = SETTINGS_ID.set(settings.id().clone());
        let _ = ABOUT_ID.set(about.id().clone());
        // Keep the native menu alive for the process lifetime. muda's wrappers are
        // !Send/!Sync so they can't live in a static; leaking them at startup is
        // the intended one-time cost.
        std::mem::forget(menu);
        std::mem::forget(app);
        std::mem::forget(about);
        std::mem::forget(settings);
    }

    /// Drain pending menu events once; report which items fired. Polled once per
    /// frame from the overlay's update loop (a single drain, so events aren't
    /// lost between separate checks).
    pub fn selected() -> Selected {
        let mut sel = Selected::default();
        while let Ok(ev) = muda::MenuEvent::receiver().try_recv() {
            if SETTINGS_ID.get() == Some(&ev.id) {
                sel.settings = true;
            }
            if ABOUT_ID.get() == Some(&ev.id) {
                sel.about = true;
            }
        }
        sel
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::Selected;

    pub fn install() {}

    pub fn selected() -> Selected {
        Selected::default()
    }
}

pub use imp::{install, selected};
