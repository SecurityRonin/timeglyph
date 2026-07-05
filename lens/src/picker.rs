//! The text of the UI element currently under the cursor, per platform.
//!
//! - **Windows**: `IUIAutomation::ElementFromPoint`.
//! - **macOS**: the Accessibility API `AXUIElementCopyElementAtPosition`.
//! - **Linux (X11)**: AT-SPI — descend `GetAccessibleAtPoint` to the deepest
//!   element under the X11 pointer, read its Text interface (or accessible name).
//!
//! Each backend exposes the same `Picker::new() -> Result<Self, String>` and
//! `text_under_cursor(&self) -> Option<String>`, so the overlay is identical.

pub use imp::{accessibility_ok, prompt_accessibility, Picker};

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern, TextUnit_Line, UIA_TextPatternId,
    };
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
                // Narrow to the LINE under the cursor via the Text pattern, so a big
                // text control (a terminal tab, an editor) yields only the line at
                // the pointer, not its whole buffer; off-screen text isn't under the
                // point, so it's excluded. Fall back to the element name when the
                // control exposes no Text pattern (a button, a custom control).
                if let Ok(pattern) =
                    element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                {
                    if let Ok(range) = pattern.RangeFromPoint(pt) {
                        let _ = range.ExpandToEnclosingUnit(TextUnit_Line);
                        if let Ok(text) = range.GetText(-1) {
                            let s = text.to_string();
                            let s = s.trim().to_string();
                            if !s.is_empty() {
                                return Some(s);
                            }
                        }
                    }
                }
                let name = element.CurrentName().ok()?.to_string();
                let name = name.trim().to_string();
                (!name.is_empty()).then_some(name)
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
                Some(off) => timeglyph_lens::scan::word_at(&text, off),
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
    use atspi::proxy::accessible::{AccessibleProxy, ObjectRefExt};
    use atspi::proxy::proxy_ext::ProxyExt;
    use atspi::zbus::block_on;
    use atspi::{AccessibilityConnection, CoordType, Granularity};

    /// The real gate is whether the accessibility bus is reachable (assistive
    /// technologies enabled) — checked when [`Picker::new`] connects.
    pub fn accessibility_ok() -> bool {
        true
    }

    /// No app-triggered permission prompt on Linux; AT-SPI is enabled via the
    /// desktop's accessibility settings.
    pub fn prompt_accessibility() {}

    pub struct Picker {
        conn: AccessibilityConnection,
    }

    impl Picker {
        pub fn new() -> Result<Self, String> {
            let conn = block_on(AccessibilityConnection::new()).map_err(|e| {
                format!(
                    "AT-SPI accessibility bus unavailable ({e}); enable assistive \
                     technologies and run under X11"
                )
            })?;
            Ok(Self { conn })
        }

        pub fn text_under_cursor(&self) -> Option<String> {
            let (x, y) = cursor_point()?;
            block_on(text_at(&self.conn, x, y))
        }
    }

    /// Walk the AT-SPI tree to the deepest accessible under the screen point and
    /// return its text (falling back to the accessible name). Screen coordinates
    /// throughout; some toolkits want window coordinates once inside a frame — a
    /// known AT-SPI quirk to revisit if a toolkit misreports.
    async fn text_at(conn: &AccessibilityConnection, x: i32, y: i32) -> Option<String> {
        let zconn = conn.connection();
        let root = conn.root_accessible_on_registry().await.ok()?;
        for app_ref in root.get_children().await.ok()? {
            if app_ref.is_null() {
                continue;
            }
            let Ok(app) = app_ref.into_accessible_proxy(zconn).await else {
                continue;
            };
            for frame_ref in app.get_children().await.unwrap_or_default() {
                if frame_ref.is_null() {
                    continue;
                }
                let Ok(frame) = frame_ref.into_accessible_proxy(zconn).await else {
                    continue;
                };
                if let Some(leaf) = deepest_at(zconn, frame, x, y).await {
                    if let Some(text) = text_of(&leaf, x, y).await {
                        return Some(text);
                    }
                }
            }
        }
        None
    }

    /// Iteratively descend via `GetAccessibleAtPoint` to the deepest child at the
    /// point; `None` when the point isn't inside `start`.
    async fn deepest_at<'c>(
        zconn: &'c atspi::zbus::Connection,
        start: AccessibleProxy<'c>,
        x: i32,
        y: i32,
    ) -> Option<AccessibleProxy<'c>> {
        let mut current = start;
        let mut descended = false;
        for _ in 0..32 {
            let Ok(proxies) = current.proxies().await else {
                break;
            };
            let Ok(component) = proxies.component().await else {
                break;
            };
            let Ok(child) = component
                .get_accessible_at_point(x, y, CoordType::Screen)
                .await
            else {
                break;
            };
            if child.is_null() {
                break;
            }
            let Ok(next) = child.into_accessible_proxy(zconn).await else {
                break;
            };
            current = next;
            descended = true;
        }
        descended.then_some(current)
    }

    /// The element's text (Text interface), else its accessible name.
    async fn text_of(node: &AccessibleProxy<'_>, x: i32, y: i32) -> Option<String> {
        if let Ok(proxies) = node.proxies().await {
            if let Ok(text) = proxies.text().await {
                // Narrow to the LINE under the cursor point, not the whole control.
                // A big text area (a terminal tab, an editor) would otherwise dump
                // its entire buffer; and off-screen scrollback sits under no screen
                // point, so `get_offset_at_point` naturally excludes it.
                if let Ok(offset) = text.get_offset_at_point(x, y, CoordType::Screen).await {
                    if offset >= 0 {
                        if let Ok((line, _, _)) =
                            text.get_string_at_offset(offset, Granularity::Line).await
                        {
                            let line = line.trim().to_string();
                            if !line.is_empty() {
                                return Some(line);
                            }
                        }
                    }
                }
                // Fallback: a toolkit that doesn't implement point→offset — read the
                // whole (small) control rather than nothing.
                if let Ok(count) = text.character_count().await {
                    if count > 0 {
                        if let Ok(s) = text.get_text(0, count).await {
                            let s = s.trim().to_string();
                            if !s.is_empty() {
                                return Some(s);
                            }
                        }
                    }
                }
            }
        }
        node.name()
            .await
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// The X11 pointer position in screen coordinates.
    fn cursor_point() -> Option<(i32, i32)> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::ConnectionExt;
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen_num)?.root;
        let pointer = conn.query_pointer(root).ok()?.reply().ok()?;
        Some((i32::from(pointer.root_x), i32::from(pointer.root_y)))
    }
}
