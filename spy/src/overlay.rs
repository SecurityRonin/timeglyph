//! The live overlay: a small always-on-top window whose label is refreshed on a
//! timer with the element under the cursor and its timeglyph readings.
//!
//! NOTE: this module compiles and runs only on Windows. Off-Windows builds
//! exclude it (`#[cfg(windows)]`), so the scan core + the text-mode binary still
//! build and test everywhere; the Win32/UIA path is verified on Windows.

use std::cell::RefCell;

use windows::core::{w, HSTRING};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, LoadCursorW, PostQuitMessage,
    RegisterClassW, SetTimer, SetWindowTextW, ShowWindow, TranslateMessage, UpdateWindow,
    CW_USEDEFAULT, IDC_ARROW, MSG, SW_SHOW, WINDOW_EX_STYLE, WM_DESTROY, WM_TIMER, WNDCLASSW,
    WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

use crate::picker::Picker;
use crate::scan;

/// Mutable window state, kept thread-local because the message loop and the
/// `WndProc` run on the same thread (and `HWND` is not `Send`).
struct State {
    picker: Picker,
    label: HWND,
    last_text: String,
}

thread_local!(static STATE: RefCell<Option<State>> = const { RefCell::new(None) });

const TIMER_ID: usize = 1;
const POLL_MS: u32 = 250;

/// Open the overlay window and run the message loop until it is closed.
pub fn run() -> windows::core::Result<()> {
    unsafe {
        let hinstance = GetModuleHandleW(None)?;
        let class = w!("TimeglyphSpyWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let window = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class,
            w!("timeglyph-spy — hover any number"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            260,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;
        let label = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("Move the cursor over a number…"),
            WS_CHILD | WS_VISIBLE,
            8,
            8,
            540,
            240,
            Some(window),
            None,
            Some(hinstance.into()),
            None,
        )?;

        let picker = Picker::new()?;
        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                picker,
                label,
                last_text: String::new(),
            });
        });

        SetTimer(Some(window), TIMER_ID, POLL_MS, None);
        let _ = ShowWindow(window, SW_SHOW);
        let _ = UpdateWindow(window);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Ok(())
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            on_tick();
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Each timer tick: read the element under the cursor and, if its text changed,
/// rebuild the label with the numbers it contains and their datetime readings.
fn on_tick() {
    STATE.with(|s| {
        let mut borrow = s.borrow_mut();
        let Some(state) = borrow.as_mut() else {
            return;
        };
        let text = state.picker.text_under_cursor().unwrap_or_default();
        if text == state.last_text {
            return;
        }
        state.last_text = text.clone();
        let body = render_body(&text);
        unsafe {
            let _ = SetWindowTextW(state.label, &HSTRING::from(body));
        }
    });
}

/// Build the overlay's text body for the element `text` under the cursor. Pure,
/// so it is unit-testable independently of Win32 (CRLF for the STATIC control).
fn render_body(text: &str) -> String {
    if text.is_empty() {
        return "(no element under cursor)".to_string();
    }
    let hits = scan::inspect_text(text, 4);
    let mut body = format!("element: {text}\r\n");
    if hits.is_empty() {
        body.push_str("\r\n(no timestamp-like number)");
        return body;
    }
    for nr in hits {
        body.push_str(&format!("\r\n{}\r\n", nr.number));
        for r in nr.readings {
            body.push_str(&format!("    {r}\r\n"));
        }
    }
    body
}
