//! The native macOS application menu with a standard `Settings…` item (⌘,),
//! following normal Mac convention. The menu bar is only shown while
//! timeglyph-lens is the frontmost app, so this complements — does not replace —
//! the in-window ⚙ button (which stays reachable while the tool is unfocused).
//!
//! No-op on other platforms (egui draws its own chrome there).

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::OnceLock;

    use muda::accelerator::{Accelerator, Code, Modifiers};
    use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

    /// Id of the `Settings…` item, set once at [`install`], matched in
    /// [`settings_selected`].
    static SETTINGS_ID: OnceLock<MenuId> = OnceLock::new();

    /// Build the standard app menu (About · Settings… ⌘, · Hide · Quit) and set it
    /// as the `NSApplication` main menu. Call once, on the main thread, after the
    /// app is initialized.
    pub fn install() {
        let menu = Menu::new();
        let app = Submenu::new("timeglyph-lens", true);
        let settings = MenuItem::new(
            "Settings…",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)),
        );
        let _ = app.append_items(&[
            &PredefinedMenuItem::about(Some("timeglyph-lens"), None),
            &PredefinedMenuItem::separator(),
            &settings,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::quit(None),
        ]);
        let _ = menu.append(&app);
        menu.init_for_nsapp();
        let _ = SETTINGS_ID.set(settings.id().clone());
        // Keep the native menu alive for the process lifetime. muda's wrappers are
        // !Send/!Sync so they can't live in a static; leaking them at startup is
        // the intended one-time cost.
        std::mem::forget(menu);
        std::mem::forget(app);
        std::mem::forget(settings);
    }

    /// Drain pending menu events; `true` if `Settings…` was chosen since the last
    /// call. Polled once per frame from the overlay's update loop.
    pub fn settings_selected() -> bool {
        let mut hit = false;
        while let Ok(ev) = muda::MenuEvent::receiver().try_recv() {
            if SETTINGS_ID.get() == Some(&ev.id) {
                hit = true;
            }
        }
        hit
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn install() {}

    pub fn settings_selected() -> bool {
        false
    }
}

pub use imp::{install, settings_selected};
