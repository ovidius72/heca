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
    if bytes.len() > limit {
        return Err(io::Error::other("native event exceeds max_report_bytes"));
    }
    let Value::Object(native) = serde_json::from_slice::<Value>(&bytes)? else {
        return Err(io::Error::other("native event must be a JSON object"));
    };
    Ok(native)
}

fn session_of(native: &serde_json::Map<String, Value>) -> Option<&str> {
    native.get("session_id").and_then(Value::as_str)
}

/// A hook outside heca is **inactive unless a session of its program was seen in a pane**: it then
/// reads the event only to find its session, and anything it cannot read is not its business — it
/// succeeds silently, as every inactive hook does.
fn receive_remembered(
    input: impl Read,
    driver: &str,
    event: &str,
    limit: usize,
    folder: &std::path::Path,
) -> io::Result<()> {
    if !super::sessions::any_for(folder, driver) {
        return Ok(());
    }
    let Ok(native) = read_native(input, limit) else {
        return Ok(());
    };
    let Some(connection) =
        session_of(&native).and_then(|id| super::sessions::find(folder, driver, id))
    else {
        return Ok(());
    };
    let reports = Reports {
        endpoint: Some(connection.endpoint),
        events: connection.events,
        limit,
    };
    if !reports.events.contains_key(event) {
        return Ok(());
    }
    forward(native, event, &reports)
}

fn receive(
    input: impl Read,
    driver: &str,
    event: &str,
    reports: &Reports,
    folder: &std::path::Path,
) -> io::Result<()> {
    if reports.endpoint.is_none() || !reports.events.contains_key(event) {
        return Ok(());
    }
    let native = read_native(input, reports.limit)?;
    // Remember where this session reports, for its hooks that run outside heca.
    if let (Some(id), Some(endpoint)) = (session_of(&native), &reports.endpoint) {
        let connection = super::sessions::Connection {
            endpoint: endpoint.clone(),
            events: reports.events.clone(),
        };
        let _ = super::sessions::remember(folder, driver, id, &connection);
    }
    forward(native, event, reports)
}

/// How much of the end of a conversation log is read to find the model of its last answer — the last
/// answer is near the end, and a log can be very long.
const TRANSCRIPT_TAIL: u64 = 256 * 1024;

/// **The model of the last answer in the conversation log at `path`** — `message.model` of the last
/// `assistant` line. A harness whose hooks name the model only at start (Claude Code) still writes
/// it on every answer, so a model changed mid-session (`/model`) shows at the next event. Read in the
/// hook's own process, never on heca's loop.
fn last_model(path: &std::path::Path) -> Option<String> {
    use std::io::{Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(TRANSCRIPT_TAIL)))
        .ok()?;
    let mut tail = String::new();
    file.take(TRANSCRIPT_TAIL).read_to_string(&mut tail).ok()?;
    tail.lines().rev().find_map(|line| {
        let entry: Value = serde_json::from_str(line).ok()?;
        (entry.get("type")? == "assistant")
            .then(|| {
                entry
                    .get("message")?
                    .get("model")?
                    .as_str()
                    .map(str::to_owned)
            })
            .flatten()
    })
}

fn forward(
    mut native: serde_json::Map<String, Value>,
    event: &str,
    reports: &Reports,
) -> io::Result<()> {
    // The model the harness is answering with now, from its own conversation log, when the hook
    // does not name it.
    if !native.get("model").is_some_and(Value::is_string)
        && let Some(model) = native
            .get("transcript_path")
            .and_then(Value::as_str)
            .and_then(|path| last_model(std::path::Path::new(path)))
    {
        native.insert("model".into(), Value::String(model));
    }
    // Native hooks also contain prompts, paths and permission settings. Only the contract's
    // session fields cross this boundary; the configured driver owns the event and status.
    native.retain(|key, _| {
        matches!(
            key.as_str(),
            "turn_id" | "model" | "session_id" | "session_title"
        )
    });
    native.insert("event".into(), Value::String(event.into()));
    reports.report(Value::Object(native))
}

#[cfg(test)]
mod tests;
