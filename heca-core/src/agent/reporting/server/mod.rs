use std::io::{self, BufRead, Read, Write};

use serde_json::{Value, json};

use super::{ReportingHelper, records::Reports};

pub(super) fn run(input: impl BufRead, output: impl Write, limit: usize) -> io::Result<()> {
    serve(input, output, &Reports::environment(limit)?)
}

fn serve(mut input: impl BufRead, mut output: impl Write, reports: &Reports) -> io::Result<()> {
    loop {
        let mut bytes = Vec::new();
        let read = (&mut input).take(reports.limit as u64 + 1).read_until(b'\n', &mut bytes)?;
        if read == 0 { return Ok(()) }
        if bytes.len() > reports.limit { return Err(io::Error::other("MCP request exceeds max_report_bytes")) }
        let response = match serde_json::from_slice::<Value>(&bytes) {
            Ok(request) => dispatch(request, reports),
            Err(_) => Some(error(Value::Null, -32700, "invalid JSON request")),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

fn dispatch(request: Value, reports: &Reports) -> Option<Value> {
    let id = request.get("id")?.clone();
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Some(error(id, -32600, "invalid JSON-RPC request"));
    }
    let result = match request.get("method").and_then(Value::as_str) {
        Some("initialize") => json!({
            "protocolVersion": "2024-11-05", "capabilities": {"tools": {}},
            "serverInfo": {"name": "heca-reporting", "version": env!("CARGO_PKG_VERSION")},
        }),
        Some("ping") => json!({}),
        Some("tools/list") => json!({"tools": [{
            "name": ReportingHelper::TOOL,
            "description": "Report a native lifecycle event to the owning Heca terminal.",
            "inputSchema": {"type": "object", "required": ["event"], "additionalProperties": false,
                "properties": {"event": {"type": "string"}, "turn_id": {"type": ["string", "null"]},
                    "model": {"type": ["string", "null"]}, "session_id": {"type": ["string", "null"]},
                    "session_title": {"type": ["string", "null"]}}},
            "annotations": {"destructiveHint": false, "openWorldHint": false},
        }]}),
        Some("tools/call") => {
            let params = &request["params"];
            if params["name"].as_str() != Some(ReportingHelper::TOOL) {
                return Some(error(id, -32602, "unknown reporting tool"));
            }
            match reports.report(params["arguments"].clone()) {
                // An empty hook result does not add model context or change native permissions.
                Ok(()) => json!({"content": [{"type": "text", "text": "{}"}]}),
                Err(error) => json!({"isError": true, "content": [{"type": "text", "text": error.to_string()}]}),
            }
        }
        _ => return Some(error(id, -32601, "unknown method")),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests;
