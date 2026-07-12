//! MCP (Model Context Protocol) stdio server — the time authority for LLM-driven
//! DFIR (LLMs botch epoch math; a tool call replaces a hallucinated conversion
//! with a cited, reproducible reading). The JSON-RPC handler is a pure function
//! (Humble Object): the `mcp` subcommand is a thin stdin/stdout loop over it, so
//! the protocol is tested here without a subprocess.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::Value;
use timeglyph::mcp::handle;

fn json(resp: &str) -> Value {
    serde_json::from_str(resp).expect("valid JSON-RPC response")
}

#[test]
fn initialize_returns_protocol_and_server_info() {
    let v = json(&handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap());
    assert_eq!(v["id"], 1);
    assert!(
        v["result"]["protocolVersion"].is_string(),
        "protocol version"
    );
    assert_eq!(v["result"]["serverInfo"]["name"], "timeglyph");
}

#[test]
fn tools_list_advertises_identify_and_explain() {
    let v = json(&handle(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#).unwrap());
    let names: Vec<&str> = v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"identify"),
        "advertises identify: {names:?}"
    );
    assert!(names.contains(&"explain"), "advertises explain: {names:?}");
}

#[test]
fn tools_call_identify_returns_the_readings_matching_the_library() {
    let req = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"identify","arguments":{"value":"1577836800"}}}"#;
    let v = json(&handle(req).unwrap());
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("unix") && text.contains("2020-01-01"),
        "identify tool returns the cited readings: {text}"
    );
    // Contract: the MCP tool output IS the library output (no divergence).
    assert!(text.contains(&timeglyph::interpret::identify_json("1577836800")));
}

#[test]
fn a_notification_produces_no_response() {
    assert!(handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
}

#[test]
fn an_unknown_method_is_a_jsonrpc_error() {
    let v = json(&handle(r#"{"jsonrpc":"2.0","id":9,"method":"bogus"}"#).unwrap());
    assert_eq!(v["error"]["code"], -32601, "method not found");
}
