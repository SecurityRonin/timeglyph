//! The live overlay (eframe/egui): an always-on-top window that follows the
//! cursor and shows the timeglyph readings for any number in the element under
//! it. Cross-platform — the same window on Windows and macOS; only the
//! [`Picker`] is platform-specific.

use std::time::Duration;

use eframe::egui;

use crate::picker::Picker;
use crate::scan;

/// Open the overlay window and run until it is closed.
pub fn run() -> Result<(), String> {
    let picker = Picker::new()?;
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([580.0, 360.0])
            .with_always_on_top()
            .with_title("timeglyph-spy"),
        ..Default::default()
    };
    eframe::run_native(
        "timeglyph-spy",
        native_options,
        Box::new(|_cc| Ok(Box::new(SpyApp::new(picker)))),
    )
    .map_err(|e| e.to_string())
}

struct SpyApp {
    picker: Picker,
    last_text: String,
    body: String,
}

impl SpyApp {
    fn new(picker: Picker) -> Self {
        Self {
            picker,
            last_text: String::new(),
            body: "Move the cursor over a number…".to_string(),
        }
    }
}

impl eframe::App for SpyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let text = self.picker.text_under_cursor().unwrap_or_default();
        if text != self.last_text {
            self.last_text.clone_from(&text);
            self.body = render_body(&text);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| ui.monospace(&self.body));
        });
        // Poll the cursor a few times a second without busy-spinning.
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

/// Build the overlay text for the element `text` under the cursor. Pure, so it
/// is testable independently of the GUI.
pub(crate) fn render_body(text: &str) -> String {
    if text.is_empty() {
        return "(no element under cursor)".to_string();
    }
    let hits = scan::inspect_text(text, 4);
    let mut body = format!("element: {text}\n");
    if hits.is_empty() {
        body.push_str("\n(no timestamp-like number)");
        return body;
    }
    for nr in hits {
        body.push_str(&format!("\n{}\n", nr.number));
        for r in nr.readings {
            body.push_str(&format!("    {r}\n"));
        }
    }
    body
}
