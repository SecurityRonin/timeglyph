//! UI Automation: the text of the UI element currently under the cursor.
//!
//! Uses `IUIAutomation::ElementFromPoint`, which reads native, Win32, WPF, and
//! UIA-exposing apps (browsers, Electron) — broader than raw `WindowFromPoint`.

use windows::core::Interface;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// A UI Automation client that resolves the element under the cursor.
pub struct Picker {
    uia: IUIAutomation,
}

impl Picker {
    /// Initialise COM (apartment-threaded, as UIA wants) and create the client.
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            // A second init on an already-initialised apartment returns
            // S_FALSE/RPC_E_CHANGED_MODE; either is fine for our single thread.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            Ok(Self { uia })
        }
    }

    /// The `Name` text of the element under the cursor, or `None` when there is
    /// no element or it exposes no name.
    #[must_use]
    pub fn text_under_cursor(&self) -> Option<String> {
        unsafe {
            let mut pt = POINT::default();
            GetCursorPos(&mut pt).ok()?;
            let element = self.uia.ElementFromPoint(pt).ok()?;
            let text = element.CurrentName().ok()?.to_string();
            (!text.is_empty()).then_some(text)
        }
    }
}
