//! Anti-drift gate: the human-facing validation doc must stay in sync with the
//! `FORMATS` registry, so a new format cannot ship undocumented and the stated
//! coverage cannot silently fall behind. A cheap regression backstop for the
//! "docs drift from the registry" bug class (the registry header comment had
//! rotted this way — listing already-shipped formats as TODO).
#![allow(clippy::unwrap_used)]

use timeglyph::registry::FORMATS;

fn validation_md() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/validation.md");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn every_registry_format_is_documented() {
    let doc = validation_md();
    let missing: Vec<&str> = FORMATS
        .iter()
        .map(|f| f.id)
        .filter(|id| !doc.contains(&format!("`{id}`")))
        .collect();
    assert!(
        missing.is_empty(),
        "formats missing from docs/validation.md: {missing:?}"
    );
}

#[test]
fn validation_md_states_the_current_format_count() {
    let doc = validation_md();
    let marker = format!("{} numeric/packed formats", FORMATS.len());
    assert!(
        doc.contains(&marker),
        "docs/validation.md must state the current registry size (\"{marker}\") so coverage can't drift"
    );
}
