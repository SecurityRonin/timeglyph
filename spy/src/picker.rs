//! The text of the UI element currently under the cursor, per platform.
//!
//! - **Windows**: `IUIAutomation::ElementFromPoint`.
//! - **macOS**: the Accessibility API `AXUIElementCopyElementAtPosition`.
//! - Other platforms: a stub that reports the live inspector is unsupported.
//!
//! Each backend exposes the same `Picker::new() -> Result<Self, String>` and
//! `text_under_cursor(&self) -> Option<String>`, so the overlay is identical.

pub use imp::Picker;

#[cfg(windows)]
mod imp {
    use windows::core::Interface;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    pub struct Picker {
        uia: IUIAutomation,
    }

    impl Picker {
        pub fn new() -> Result<Self, String> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let uia: IUIAutomation =
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                        .map_err(|e| e.to_string())?;
                Ok(Self { uia })
            }
        }

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
}

#[cfg(target_os = "macos")]
mod imp {
    use accessibility_sys::{
        kAXDescriptionAttribute, kAXErrorSuccess, kAXTitleAttribute, kAXValueAttribute,
        AXUIElementCopyAttributeValue, AXUIElementCopyElementAtPosition,
        AXUIElementCreateSystemWide, AXUIElementRef,
    };
    use core_foundation::base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringGetTypeID, CFStringRef};
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    pub struct Picker {
        system_wide: AXUIElementRef,
    }

    // The system-wide AX element is a process-global handle; sharing it across
    // the single-threaded overlay loop is sound.
    unsafe impl Send for Picker {}

    impl Picker {
        pub fn new() -> Result<Self, String> {
            let system_wide = unsafe { AXUIElementCreateSystemWide() };
            if system_wide.is_null() {
                return Err("could not create the system-wide AX element (grant Accessibility access in System Settings → Privacy & Security)".into());
            }
            Ok(Self { system_wide })
        }

        pub fn text_under_cursor(&self) -> Option<String> {
            let (x, y) = cursor_point()?;
            let mut element: AXUIElementRef = std::ptr::null_mut();
            let err = unsafe {
                AXUIElementCopyElementAtPosition(self.system_wide, x as f32, y as f32, &mut element)
            };
            if err != kAXErrorSuccess || element.is_null() {
                return None;
            }
            let text = attr_string(element, kAXValueAttribute)
                .or_else(|| attr_string(element, kAXTitleAttribute))
                .or_else(|| attr_string(element, kAXDescriptionAttribute));
            unsafe { CFRelease(element.cast()) };
            text.filter(|s| !s.is_empty())
        }
    }

    /// The cursor position in top-left screen coordinates.
    fn cursor_point() -> Option<(f64, f64)> {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
        let event = CGEvent::new(source).ok()?;
        let loc = event.location();
        Some((loc.x, loc.y))
    }

    /// Read a string-valued AX attribute, or `None` if absent or not a string.
    fn attr_string(element: AXUIElementRef, attr: &str) -> Option<String> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyAttributeValue(element, cf_attr.as_concrete_TypeRef(), &mut value)
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let result = unsafe {
            if CFGetTypeID(value) == CFStringGetTypeID() {
                Some(CFString::wrap_under_get_rule(value as CFStringRef).to_string())
            } else {
                None
            }
        };
        unsafe { CFRelease(value) };
        result
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub struct Picker;

    impl Picker {
        pub fn new() -> Result<Self, String> {
            Err("the live cursor inspector is only available on Windows and macOS".into())
        }

        pub fn text_under_cursor(&self) -> Option<String> {
            None
        }
    }
}
