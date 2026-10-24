use std::{collections::BTreeMap, fs, io::Cursor, path::PathBuf};

use super::*;
use crate::agent::{AgentState, AgentStatus};

fn call(reports: &Reports, arguments: Value) -> Value {
    dispatch(json!({"jsonrpc":"2.0", "id":1, "method":"tools/call",
        "params":{"name":ReportingHelper::TOOL, "arguments":arguments}}), reports).unwrap()
}

fn reports(root: &std::path::Path, name: &str, limit: usize) -> Reports {
    let endpoint = root.join(name);
    fs::write(&endpoint, []).unwrap();
    Reports { endpoint: Some(endpoint), events: [(
        "Stop".into(), AgentState::new("custom", "Custom program", AgentStatus::new("finished")),
    )].into(), limit }
}

#[test]
fn independent_connections_report_only_to_their_own_terminal() {
    let root = tempfile::tempdir().unwrap();
    let first = reports(root.path(), "first", 4096);
    let second = reports(root.path(), "second", 4096);
    for (connection, turn) in [(&first, "turn-a"), (&second, "turn-b"), (&first, "turn-c")] {
        assert!(call(connection, json!({"event":"Stop", "turn_id":turn}))["result"]["isError"].is_null());
    }
    let turns = |path: &PathBuf| fs::read_to_string(path).unwrap().lines().map(|line| {
        serde_json::from_str::<AgentState>(line).unwrap().turn_id.unwrap()
    }).collect::<Vec<_>>();
    assert_eq!(turns(first.endpoint.as_ref().unwrap()), ["turn-a", "turn-c"]);
    assert_eq!(turns(second.endpoint.as_ref().unwrap()), ["turn-b"]);
}

#[test]
fn inactive_events_and_closed_terminals_are_silent() {
    let root = tempfile::tempdir().unwrap();
    let active = reports(root.path(), "reports", 4096);
    call(&active, json!({"event":"Disabled"}));
    assert!(fs::read(active.endpoint.as_ref().unwrap()).unwrap().is_empty());
    fs::remove_file(active.endpoint.as_ref().unwrap()).unwrap();
    call(&active, json!({"event":"Stop"}));
    assert!(!active.endpoint.as_ref().unwrap().exists());
    let inactive = Reports { endpoint: None, events: BTreeMap::new(), limit: 4096 };
    assert!(call(&inactive, json!({"event":"Stop"}))["result"]["isError"].is_null());
}

#[test]
fn metadata_roundtrips_without_accepting_native_status_or_driver_overrides() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), "metadata", 4096);
    let metadata = json!({"event":"Stop", "model":"model-a", "session_id":"session-a", "session_title":"Title"});
    assert!(call(&reports, metadata)["result"]["isError"].is_null());
    let bytes = fs::read(reports.endpoint.as_ref().unwrap()).unwrap();
    let report: AgentState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report.model.as_deref(), Some("model-a"));
    assert_eq!(report.session_id.as_deref(), Some("session-a"));
    assert_eq!(report.session_title.as_deref(), Some("Title"));
    for arguments in [json!({"event":"Stop", "session_id":4}), json!({"event":"Stop", "driver":"foreign"}),
        json!({"event":"Stop", "status":"working"}), json!({"event":"Stop", "model":"x".repeat(4096)})] {
        assert_eq!(call(&reports, arguments)["result"]["isError"], true);
    }
    assert_eq!(fs::read(reports.endpoint.as_ref().unwrap()).unwrap(), bytes);
}

#[test]
fn malformed_events_and_oversized_reports_do_not_write() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), "reports", 128);
    for arguments in [json!({"status":"working"}), json!({"event":"Stop", "turn_id":5}),
        json!({"event":"Stop", "turn_id":"x".repeat(128)})] {
        assert_eq!(call(&reports, arguments)["result"]["isError"], true);
    }
    assert!(fs::read(reports.endpoint.as_ref().unwrap()).unwrap().is_empty());
}

#[test]
fn stdio_handles_handshake_notifications_errors_and_the_configured_limit() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), "reports", 4096);
    let input = [json!({"jsonrpc":"2.0", "id":1, "method":"initialize"}),
        json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0", "id":2, "method":"tools/list"}),
        json!({"jsonrpc":"2.0", "id":3, "method":"unknown"})]
        .map(|value| value.to_string()).join("\n") + "\ninvalid\n";
    let mut output = Vec::new();
    serve(Cursor::new(input), &mut output, &reports).unwrap();
    let replies: Vec<Value> = String::from_utf8(output).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(replies.len(), 4);
    assert_eq!(replies[0]["result"]["capabilities"], json!({"tools":{}}));
    assert_eq!(replies[1]["result"]["tools"][0]["name"], ReportingHelper::TOOL);
    assert_eq!(replies[2]["error"]["code"], -32601);
    assert_eq!(replies[3]["error"]["code"], -32700);
    assert!(serve(Cursor::new(vec![b'x'; 4097]), Vec::new(), &reports).is_err());
}
