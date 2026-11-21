//! One-shot command hooks share the MCP reporter's validation and bounded output.

use std::io::{self, Read};

use serde_json::Value;

use super::{DRIVER, ENDPOINT, records::Reports};

pub(super) fn run(input: impl Read, driver: &str, event: &str, limit: usize) -> io::Result<()> {
    if driver.is_empty() {
        return Ok(());
    }
    let folder = super::sessions::folder();
    let has_connection = std::env::var(DRIVER).ok().as_deref() == Some(driver)
        && std::env::var_os(ENDPOINT).is_some_and(|value| !value.is_empty());
    if has_connection {
        receive(input, driver, event, &Reports::environment(limit)?, &folder)
    } else {
        // **A hook outside heca** — a session whose work runs in a process heca never started:
        // report to the pane that session was seen in, if any.
        receive_remembered(input, driver, event, limit, &folder)
    }
}

/// Read the native event, bounded by `limit`.
fn read_native(input: impl Read, limit: usize) -> io::Result<serde_json::Map<String, Value>> {
    let mut bytes = Vec::new();
    input.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit { return Err(io::Error::other("native event exceeds max_report_bytes")) }
    let Value::Object(native) = serde_json::from_slice::<Value>(&bytes)? else {
        return Err(io::Error::other("native event must be a JSON object"));
    };
    Ok(native)
}

fn session_of(native: &serde_json::Map<String, Value>) -> Option<&str> {
    native.get("session_id").and_then(Value::as_str)
}

fn receive_remembered(input: impl Read, driver: &str, event: &str, limit: usize, folder: &std::path::Path) -> io::Result<()> {
    let native = read_native(input, limit)?;
    let Some(connection) = session_of(&native).and_then(|id| super::sessions::find(folder, driver, id)) else {
        return Ok(());
    };
    let reports = Reports { endpoint: Some(connection.endpoint), events: connection.events, limit };
    if !reports.events.contains_key(event) { return Ok(()) }
    forward(native, event, &reports)
}

fn receive(input: impl Read, driver: &str, event: &str, reports: &Reports, folder: &std::path::Path) -> io::Result<()> {
    if reports.endpoint.is_none() || !reports.events.contains_key(event) { return Ok(()) }
    let native = read_native(input, reports.limit)?;
    // Remember where this session reports, for its hooks that run outside heca.
    if let (Some(id), Some(endpoint)) = (session_of(&native), &reports.endpoint) {
        let connection = super::sessions::Connection { endpoint: endpoint.clone(), events: reports.events.clone() };
        let _ = super::sessions::remember(folder, driver, id, &connection);
    }
    forward(native, event, reports)
}

fn forward(mut native: serde_json::Map<String, Value>, event: &str, reports: &Reports) -> io::Result<()> {
    // Native hooks also contain prompts, paths and permission settings. Only the contract's
    // session fields cross this boundary; the configured driver owns the event and status.
    native.retain(|key, _| matches!(key.as_str(), "turn_id" | "model" | "session_id" | "session_title"));
    native.insert("event".into(), Value::String(event.into()));
    reports.report(Value::Object(native))
}

#[cfg(test)]
mod tests;
