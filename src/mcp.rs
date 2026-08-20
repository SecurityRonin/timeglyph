//! MCP (Model Context Protocol) JSON-RPC handler over stdio. Exposes the engine
//! as tools — `identify`, `decode`, `explain` — so an LLM-driven DFIR workflow
//! gets a cited, reproducible reading instead of a hallucinated epoch conversion
//! (LLMs are reliably wrong at FILETIME↔Unix arithmetic). The handler is a pure
//! function; the `mcp` subcommand is a thin stdin→[`handle`](crate::mcp::handle)→stdout loop over it.

use serde_json::{json, Value};

/// The MCP protocol revision this server speaks.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// The tool definitions advertised by `tools/list`.
fn tool_defs() -> Value {
    json!([
        {
            "name": "identify",
            "description": "Identify a timestamp VALUE across every format — ranked, scored, cited readings (a raw value is usually underdetermined, so this returns all plausible readings, not one verdict).",
            "inputSchema": { "type": "object", "properties": { "value": { "type": "string", "description": "the raw value (integer, float, or hex/string form)" } }, "required": ["value"] }
        },
        {
            "name": "decode",
            "description": "Decode a value under ONE known format id (see the `explain` tool or the format list).",
            "inputSchema": { "type": "object", "properties": { "format": { "type": "string" }, "value": { "type": "string" } }, "required": ["format", "value"] }
        },
        {
            "name": "explain",
            "description": "Explain a format: a spec card (epoch, tick unit, tz/leap semantics, valid range, known sentinels, citation).",
            "inputSchema": { "type": "object", "properties": { "format": { "type": "string" } }, "required": ["format"] }
        }
    ])
}

/// Execute a `tools/call`. Returns the MCP `content` result, or a JSON-RPC
/// `(code, message)` error.
fn call_tool(params: Option<&Value>) -> Result<Value, (i64, &'static str)> {
    let params = params.ok_or((-32602, "missing params"))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing tool name"))?;
    let args = params
        .get("arguments")
        .cloned()
        // Absent arguments → Null; `.get()` on Null is None → a clean
        // "missing 'x'" error below (no fallback closure to leave uncovered).
        .unwrap_or_default();
    let str_arg = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_owned);
    let text = match name {
        "identify" => {
            let value = str_arg("value").ok_or((-32602, "missing 'value'"))?;
            crate::interpret::identify_json(&value)
        }
        "explain" => {
            let format = str_arg("format").ok_or((-32602, "missing 'format'"))?;
            crate::interpret::explain(&format).ok_or((-32602, "unknown format id"))?
        }
        "decode" => {
            let format = str_arg("format").ok_or((-32602, "missing 'format'"))?;
            let value = str_arg("value").ok_or((-32602, "missing 'value'"))?;
            let f = crate::format(&format).map_err(|_| (-32602, "unknown format id"))?;
            let inst = if let Ok(v) = value.parse::<i64>() {
                f.decode_int(v)
            } else if let Ok(v) = value.parse::<f64>() {
                f.decode_float(v)
            } else {
                return Err((-32602, "value is not numeric"));
            };
            inst.ok()
                .and_then(crate::PosixNs::to_rfc3339)
                .ok_or((-32602, "value out of decodable range"))?
        }
        _ => return Err((-32601, "unknown tool")),
    };
    Ok(json!({ "content": [ { "type": "text", "text": text } ] }))
}

/// Process one JSON-RPC message and return the response JSON string, or `None` for
/// a notification (no `id`) — which expects no reply. Never panics: a malformed or
/// unknown message yields a JSON-RPC error, never a crash.
#[must_use]
pub fn handle(request: &str) -> Option<String> {
    let req: Value = serde_json::from_str(request).ok()?;
    // A notification has no `id` and gets no response.
    let id = req.get("id").cloned()?;
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let outcome: Result<Value, (i64, &'static str)> = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "timeglyph", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => Ok(json!({ "tools": tool_defs() })),
        "tools/call" => call_tool(req.get("params")),
        _ => Err((-32601, "method not found")),
    };
    Some(match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
        Err((code, message)) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
                .to_string()
        }
    })
}
