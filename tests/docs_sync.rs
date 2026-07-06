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
fn every_format_is_exercised_by_a_test() {
    // Backstop: every registry format must be referenced by at least one test —
    // a differential oracle row (oracle.rs / encode_packed_g*.rs, cross-checked
    // against time-decode/unfurl), a spec worked-example anchor (anchors.rs), or
    // a decode/round-trip test. A new format cannot ship untested. (Per-format
    // evidence *tier* is documented in docs/validation.md, not gated here.)
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");
    let mut corpus = String::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        // Skip this gate file (it names no format) — everything else counts.
        if path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_some_and(|n| n != "docs_sync.rs")
        {
            corpus.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            corpus.push('\n');
        }
    }
    let missing: Vec<&str> = FORMATS
        .iter()
        .map(|f| f.id)
        .filter(|id| !corpus.contains(&format!("\"{id}\"")))
        .collect();
    assert!(missing.is_empty(), "formats with no test: {missing:?}");
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
