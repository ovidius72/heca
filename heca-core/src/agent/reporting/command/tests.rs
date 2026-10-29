use std::{fs, io::Cursor};

use serde_json::json;

use super::*;
use crate::agent::{AgentState, AgentStatus};

fn reports(root: &std::path::Path) -> Reports {
    let endpoint = root.join("reports");
    fs::write(&endpoint, []).unwrap();
    Reports { endpoint: Some(endpoint), events: [(
        "Stop".into(), AgentState::new("example", "Example", AgentStatus::new("finished")).event("Stop"),
    )].into(), limit: 4096 }
}

#[test]
fn native_payload_forwards_only_metadata_and_cannot_override_the_configured_report() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path());
    let native = json!({"model":"native-model", "session_id":"session-a", "session_title":"My session",
        "turn_id":"turn-a", "prompt":"private prompt", "transcript_path":"/do/not/read",
        "permission_mode":"auto", "driver":"foreign", "status":"working", "event":"Disabled"});
    receive(Cursor::new(native.to_string()), "Stop", &reports).unwrap();
    let bytes = fs::read(reports.endpoint.as_ref().unwrap()).unwrap();
    let report: AgentState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report.driver, "example");
    assert_eq!(report.status.as_str(), "finished");
    assert_eq!(report.event.as_deref(), Some("Stop"));
    assert_eq!(report.model.as_deref(), Some("native-model"));
    assert_eq!(report.session_id.as_deref(), Some("session-a"));
    assert_eq!(report.session_title.as_deref(), Some("My session"));
    assert_eq!(report.turn_id.as_deref(), Some("turn-a"));
    assert!(!String::from_utf8(bytes).unwrap().contains("private prompt"));
}

#[test]
fn disabled_events_do_not_read_input_and_closed_endpoints_are_not_recreated() {
    struct Unreadable;
    impl Read for Unreadable {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("inactive hook read stdin") }
    }
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path());
    receive(Unreadable, "Disabled", &reports).unwrap();
    assert!(fs::read(reports.endpoint.as_ref().unwrap()).unwrap().is_empty());
    fs::remove_file(reports.endpoint.as_ref().unwrap()).unwrap();
    receive(Cursor::new("{}"), "Stop", &reports).unwrap();
    assert!(!reports.endpoint.as_ref().unwrap().exists());
}

#[test]
fn invalid_native_metadata_and_oversized_inputs_do_not_write() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path());
    for native in ["not-json".into(), "[]".into(), json!({"session_id":5}).to_string(),
        json!({"model":{"id":"invalid"}}).to_string(), " ".repeat(reports.limit + 1)] {
        assert!(receive(Cursor::new(native), "Stop", &reports).is_err());
    }
    assert!(fs::read(reports.endpoint.as_ref().unwrap()).unwrap().is_empty());
}
