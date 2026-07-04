//! `timeglyph-spy` — a Spy++-style Windows inspector. Hover any UI element and
//! see timeglyph's ranked datetime readings for any number it contains.
//!
//! This crate is the GUI shell (overlay, AX/UIA picker, native menu, fonts, map
//! rendering). The pure decode/scan/presentation logic lives in the `timeglyph`
//! engine; `scan` is re-exported here so existing `timeglyph_spy::scan` paths
//! keep resolving.

pub use timeglyph::scan;

pub mod fonts;
pub mod ganzhi;
pub mod text;
pub mod theme;
pub mod tzinfo;
pub mod tzmap;
pub mod zone;
