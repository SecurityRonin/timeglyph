//! `timeglyph-core` — zero-dependency epoch arithmetic.
//!
//! The integer half of [`timeglyph`](https://docs.rs/timeglyph). Turning a
//! stored tick count into nanoseconds since the Unix epoch is a subtraction and
//! a multiplication; it needs no calendar. Rendering that instant as ISO-8601
//! does, so rendering stays upstairs. That seam is what lets this crate keep
//! zero dependencies and a 1.75 MSRV.
