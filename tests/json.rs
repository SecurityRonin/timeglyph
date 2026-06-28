//! `--json` / serde-output contract.
//!
//! The default human output is ranked candidates; the machine output must be the
//! SAME information, losslessly — a real `serde_json` serialization of each
//! `Candidate`, not a hand-rolled count stub. These tests pin the JSON shape so
//! downstream tooling (and the differential oracle harness) can consume it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use timeglyph::interpret;

#[test]
fn candidate_serializes_to_json_with_named_fields() {
    let cands = interpret::interpret_int(1_577_836_800);
    let unix = cands
        .iter()
        .find(|c| c.format_id == "unix")
        .expect("unix candidate");

    let v: serde_json::Value = serde_json::to_value(unix).expect("Candidate: Serialize");

    assert_eq!(v["format_id"], "unix");
    assert_eq!(v["instant"], 1_577_836_800_000_000_000i64); // PosixNs is transparent ns
    assert!(v["rendered"].as_str().unwrap().starts_with("2020-01-01"));
    assert!(v["score"].is_number());
    assert!(v["components"].is_array());
    assert!(v["assumptions"].is_array());
}

#[test]
fn interpret_int_result_is_a_json_array() {
    let cands = interpret::interpret_int(1_577_836_800);
    let s = serde_json::to_string(&cands).expect("Vec<Candidate>: Serialize");
    let parsed: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
    assert!(parsed.is_array());
    assert!(parsed.as_array().unwrap().len() >= 2);
}
