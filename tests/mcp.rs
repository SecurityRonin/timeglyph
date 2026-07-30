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

#[test]
fn mcp_subcommand_answers_initialize_over_stdio() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_timeglyph"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("protocolVersion") && s.contains("timeglyph"),
        "the mcp subcommand replies to initialize over stdio: {s}"
    );
}

#[test]
fn tools_call_explain_returns_the_spec_card_for_a_known_format() {
    let req = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"explain","arguments":{"format":"unix"}}}"#;
    let v = json(&handle(req).unwrap());
    let text = v["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("epoch:"),
        "the spec card carries the epoch: {text}"
    );
    // Contract: the MCP tool output IS the library output (no divergence).
    assert_eq!(text, timeglyph::interpret::explain("unix").unwrap());
}

#[test]
fn tools_call_explain_rejects_an_unknown_format_and_a_missing_argument() {
    let unknown = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"explain","arguments":{"format":"no_such_format"}}}"#;
    let v = json(&handle(unknown).unwrap());
    assert_eq!(v["error"]["code"], -32602);
    assert_eq!(v["error"]["message"], "unknown format id");

    let missing = r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"explain","arguments":{}}}"#;
    let v = json(&handle(missing).unwrap());
    assert_eq!(v["error"]["message"], "missing 'format'");
}

#[test]
fn tools_call_decode_reads_a_value_under_one_named_format() {
    // Integer path: FILETIME 100-ns ticks since 1601 for 2020-01-01T00:00:00Z.
    let req = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"decode","arguments":{"format":"filetime","value":"132223104000000000"}}}"#;
    let v = json(&handle(req).unwrap());
    assert_eq!(
        v["result"]["content"][0]["text"], "2020-01-01T00:00:00Z",
        "{v}"
    );
    // Float path: OLE automation date 43831.0 = 2020-01-01 (days since 1899-12-30).
    let req = r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"decode","arguments":{"format":"ole","value":"43831.0"}}}"#;
    let v = json(&handle(req).unwrap());
    assert_eq!(
        v["result"]["content"][0]["text"], "2020-01-01T00:00:00Z",
        "{v}"
    );
}

#[test]
fn tools_call_decode_names_each_way_it_can_refuse() {
    let call = |args: &str| {
        let req = format!(
            r#"{{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{{"name":"decode","arguments":{args}}}}}"#
        );
        json(&handle(&req).unwrap())["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(call(r"{}"), "missing 'format'");
    assert_eq!(call(r#"{"format":"unix"}"#), "missing 'value'");
    assert_eq!(
        call(r#"{"format":"no_such_format","value":"0"}"#),
        "unknown format id"
    );
    assert_eq!(
        call(r#"{"format":"unix","value":"not-a-number"}"#),
        "value is not numeric"
    );
    // Decodable as an integer, but the instant is outside the civil range.
    assert_eq!(
        call(r#"{"format":"unix","value":"9223372036854775807"}"#),
        "value out of decodable range"
    );
}

#[test]
fn an_unknown_tool_name_is_a_jsonrpc_method_not_found() {
    let req = r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}"#;
    let v = json(&handle(req).unwrap());
    assert_eq!(v["error"]["code"], -32601);
    assert_eq!(v["error"]["message"], "unknown tool");
}
