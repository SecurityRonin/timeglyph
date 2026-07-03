//! `timeglyph-spy` — a Spy++-style Windows inspector. Hover any UI element and
//! see timeglyph's ranked datetime readings for any number it contains.
//!
//! This crate splits cleanly (Humble Object): the [`scan`] module is pure,
//! cross-platform, and unit-tested; the Win32 + UI-Automation shell (the
//! element picker and the layered overlay window) lives behind `#[cfg(windows)]`
//! in the binary.

pub mod fonts;
pub mod ganzhi;
pub mod scan;
pub mod text;
pub mod tzinfo;
pub mod tzmap;
pub mod zone;
