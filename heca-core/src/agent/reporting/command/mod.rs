//! One-shot command hooks share the MCP reporter's validation and bounded output.

use std::io::{self, Read};

use serde_json::Value;

use super::{DRIVER, ENDPOINT, records::Reports};

pub(super) fn run(input: impl Read, driver: &str, event: &str, limit: usize) -> io::Result<()> {
    if driver.is_empty() || std::env::var(DRIVER).ok().as_deref() != Some(driver)
        || std::env::var_os(ENDPOINT).is_none_or(|value| value.is_empty()) {
        return Ok(());
    }
    receive(input, event, &Reports::environment(limit)?)
}

fn receive(input: impl Read, event: &str, reports: &Reports) -> io::Result<()> {
    if reports.endpoint.is_none() || !reports.events.contains_key(event) { return Ok(()) }
    let mut bytes = Vec::new();
    input.take(reports.limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > reports.limit { return Err(io::Error::other("native event exceeds max_report_bytes")) }
    let Value::Object(mut native) = serde_json::from_slice::<Value>(&bytes)? else {
        return Err(io::Error::other("native event must be a JSON object"));
    };
    // Native hooks also contain prompts, paths and permission settings. Only the contract's
    // session fields cross this boundary; the configured driver owns the event and status.
    native.retain(|key, _| matches!(key.as_str(), "turn_id" | "model" | "session_id" | "session_title"));
    native.insert("event".into(), Value::String(event.into()));
    reports.report(Value::Object(native))
}

#[cfg(test)]
mod tests;
