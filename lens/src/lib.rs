//! `timeglyph-lens` — a Spy++-style Windows inspector. Hover any UI element and
//! see timeglyph's ranked datetime readings for any number it contains.
//!
//! This crate is the GUI shell (overlay, AX/UIA picker, native menu, fonts, map
//! rendering). The pure decode/scan/presentation logic lives in the `timeglyph`
//! engine; `scan` is re-exported here so existing `timeglyph_lens::scan` paths
//! keep resolving.

pub use timeglyph::scan;

/// How many readings the overlay shows per number: **all** confident (in-window)
/// ones. The readings pane scrolls, so there is no display reason to truncate —
/// the show-all principle means likelihood ranks the readings, it never filters
/// them to a top-N slice (the analyst decides which are useful). Passed as the
/// per-number cap to [`scan::inspect_text`].
pub const READINGS_SHOWN: usize = usize::MAX;

pub mod fonts;
pub mod ganzhi;
pub mod ocr;
pub mod settings;
pub mod text;
pub mod theme;
pub mod tzinfo;
pub mod tzmap;
pub mod zone;
