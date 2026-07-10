//! The engine sources its format *knowledge* from forensicnomicon.
//!
//! After the knowledge/engine split, `timeglyph::registry::FORMATS` is built from
//! `forensicnomicon::temporal_formats::TIME_FORMATS`: every engine `Format` carries
//! a `.meta` pointing at the authoritative catalog entry, so its id/label/citation
//! are the forensicnomicon values verbatim. These assertions pin that wiring — and
//! that the packed codec is still connected (FAT round-trips).
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[test]
fn format_meta_is_the_forensicnomicon_catalog_entry() {
    let f = timeglyph::format("filetime").unwrap();
    let known = forensicnomicon::temporal_formats::time_format("filetime").unwrap();
    // The engine's metadata IS the forensicnomicon catalog entry (same pointer).
    assert!(std::ptr::eq(f.meta, known));
    assert_eq!(f.meta.citation, known.citation);
    assert_eq!(f.meta.label, known.label);
}

#[test]
fn every_engine_format_has_a_catalog_entry_with_matching_citation() {
    for f in timeglyph::registry::FORMATS.iter() {
        let known = forensicnomicon::temporal_formats::time_format(f.meta.id)
            .expect("engine format must exist in the forensicnomicon catalog");
        assert_eq!(
            f.meta.citation, known.citation,
            "citation drift for {}",
            f.meta.id
        );
    }
}

#[test]
fn packed_fat_still_round_trips_through_the_engine_codec() {
    // FAT/DOS is a Packed format: the layout tag lives in forensicnomicon, the
    // codec in the engine. Decoding then re-encoding must be identity.
    let fat = timeglyph::format("fat").unwrap();
    // Oracle-verified: 1_391_422_645 decodes to 2021-07-15T13:37:42 (see anchors.rs).
    let value = 1_391_422_645;
    let instant = fat.decode_int(value).unwrap();
    assert_eq!(
        instant.to_rfc3339().as_deref(),
        Some("2021-07-15T13:37:42Z")
    );
    assert_eq!(fat.encode_int(instant).unwrap(), value);
}
