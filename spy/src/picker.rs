//! The text of the UI element currently under the cursor, per platform.
//!
//! - **Windows**: `IUIAutomation::ElementFromPoint`.
//! - **macOS**: the Accessibility API `AXUIElementCopyElementAtPosition`.
//! - Other platforms: a stub that reports the live inspector is unsupported.
//!
//! Each backend exposes the same `Picker::new() -> Result<Self, String>` and
//! `text_under_cursor(&self) -> Option<String>`, so the overlay is identical.

pub use imp::{accessibility_ok, prompt_accessibility, Picker};

#[cfg(windows)]
mod imp {
    use windows::core::Interface;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    /// UI Automation needs no special permission grant, so the picker is always
    /// ready. (Reading an *elevated* window still requires an elevated process,
    /// but that is not a per-app permission we can request.)
    pub fn accessibility_ok() -> bool {
        true
    }

    /// No system permission prompt on Windows.
    pub fn prompt_accessibility() {}

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
        kAXDescriptionAttribute, kAXErrorSuccess, kAXRangeForPositionParameterizedAttribute,
        kAXTitleAttribute, kAXTrustedCheckOptionPrompt, kAXValueAttribute, kAXValueTypeCFRange,
        kAXValueTypeCGPoint, AXIsProcessTrusted, AXIsProcessTrustedWithOptions,
        AXUIElementCopyAttributeValue, AXUIElementCopyElementAtPosition,
        AXUIElementCopyParameterizedAttributeValue, AXUIElementCreateSystemWide, AXUIElementRef,
        AXValueCreate, AXValueGetValue, AXValueRef,
    };
    use core_foundation::base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::{CFString, CFStringGetTypeID, CFStringRef};

    /// True when this process is trusted for the macOS Accessibility API — the
    /// grant the picker needs. When false the overlay shows no readings, so it
    /// surfaces a "grant Accessibility" reminder instead.
    pub fn accessibility_ok() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// Show the system Accessibility prompt if this process isn't trusted yet
    /// (idempotent — a no-op once granted). Called once at startup so a
    /// first-time user is guided straight to the grant.
    pub fn prompt_accessibility() {
        let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
        let opts = CFDictionary::from_CFType_pairs(&[(
            key.as_CFType(),
            CFBoolean::true_value().as_CFType(),
        )]);
        unsafe {
            AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef());
        }
    }
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    /// `CFRange` (CFIndex = isize) — the `AXRangeForPosition` result payload.
    #[repr(C)]
    struct CFRange {
        location: isize,
        length: isize,
    }

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
            let full = attr_string(element, kAXValueAttribute)
                .or_else(|| attr_string(element, kAXTitleAttribute))
                .or_else(|| attr_string(element, kAXDescriptionAttribute));
            // Narrow to just the token under the cursor when the element supports
            // position→range hit-testing (text views — a terminal, an editor);
            // otherwise (labels, buttons) keep the whole value. This stops a
            // hovered iTerm tab from dumping its entire buffer's timestamps.
            let result = full.and_then(|text| match char_offset_at_point(element, x, y) {
                Some(off) => timeglyph_spy::scan::word_at(&text, off),
                None => Some(text),
            });
            unsafe { CFRelease(element.cast()) };
            result.filter(|s| !s.is_empty())
        }
    }

    /// The UTF-16 character offset in `element`'s text at screen point `(x, y)`,
    /// via the `AXRangeForPosition` parameterized attribute. `None` when the
    /// element doesn't support position hit-testing (most non-text elements).
    fn char_offset_at_point(element: AXUIElementRef, x: f64, y: f64) -> Option<usize> {
        let point = CGPoint { x, y };
        let pt_val: AXValueRef =
            unsafe { AXValueCreate(kAXValueTypeCGPoint, (&point as *const CGPoint).cast()) };
        if pt_val.is_null() {
            return None;
        }
        let attr = CFString::new(kAXRangeForPositionParameterizedAttribute);
        let mut range_ref: CFTypeRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyParameterizedAttributeValue(
                element,
                attr.as_concrete_TypeRef(),
                pt_val.cast(),
                &mut range_ref,
            )
        };
        unsafe { CFRelease(pt_val.cast()) };
        if err != kAXErrorSuccess || range_ref.is_null() {
            return None;
        }
        let mut range = CFRange {
            location: 0,
            length: 0,
        };
        let ok = unsafe {
            AXValueGetValue(
                range_ref as AXValueRef,
                kAXValueTypeCFRange,
                (&mut range as *mut CFRange).cast(),
            )
        };
        unsafe { CFRelease(range_ref) };
        if !ok || range.location < 0 {
            return None;
        }
        Some(range.location as usize)
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
    pub fn accessibility_ok() -> bool {
        true
    }

    pub fn prompt_accessibility() {}

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
