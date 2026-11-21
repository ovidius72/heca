use std::{fs, io::Cursor};

use serde_json::json;

use super::*;
use crate::agent::{AgentState, AgentStatus};

fn reports(root: &std::path::Path) -> Reports {
    let endpoint = root.join("reports");
    fs::write(&endpoint, []).unwrap();
    Reports {
        endpoint: Some(endpoint),
        events: [(
            "Stop".into(),
            AgentState::new("example", "Example", AgentStatus::new("finished")).event("Stop"),
        )]
        .into(),
        limit: 4096,
    }
}

#[test]
fn native_payload_forwards_only_metadata_and_cannot_override_the_configured_report() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path());
    let native = json!({"model":"native-model", "session_id":"session-a", "session_title":"My session",
        "turn_id":"turn-a", "prompt":"private prompt", "transcript_path":"/do/not/read",
        "permission_mode":"auto", "driver":"foreign", "status":"working", "event":"Disabled"});
    receive(
        Cursor::new(native.to_string()),
        "example",
        "Stop",
        &reports,
        &root.path().join("sessions"),
    )
    .unwrap();
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
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("inactive hook read stdin")
        }
    }
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path());
    receive(
        Unreadable,
        "example",
        "Disabled",
        &reports,
        &root.path().join("sessions"),
    )
    .unwrap();
    assert!(
        fs::read(reports.endpoint.as_ref().unwrap())
            .unwrap()
            .is_empty()
    );
    fs::remove_file(reports.endpoint.as_ref().unwrap()).unwrap();
    receive(
        Cursor::new("{}"),
        "example",
        "Stop",
        &reports,
        &root.path().join("sessions"),
    )
    .unwrap();
    assert!(!reports.endpoint.as_ref().unwrap().exists());
}

#[test]
fn invalid_native_metadata_and_oversized_inputs_do_not_write() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path());
    for native in [
        "not-json".into(),
        "[]".into(),
        json!({"session_id":5}).to_string(),
        json!({"model":{"id":"invalid"}}).to_string(),
        " ".repeat(reports.limit + 1),
    ] {
        assert!(
            receive(
                Cursor::new(native),
                "example",
                "Stop",
                &reports,
                &root.path().join("sessions")
            )
            .is_err()
        );
    }
    assert!(
        fs::read(reports.endpoint.as_ref().unwrap())
            .unwrap()
            .is_empty()
    );
}

/// **A session whose work runs outside heca still reports to its pane.** A hook with heca's
/// connection remembers where the session reports; a hook of the same session without it — Claude
/// Code's background daemon — reports there too. A pane that is gone takes its entry with it, and an
/// id that is not a plain identifier is never used as a file name.
#[test]
fn a_hook_outside_heca_reports_to_the_pane_its_session_was_seen_in() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("sessions");
    let reports = reports(root.path());
    let endpoint = reports.endpoint.clone().unwrap();
    // The pane's own hook (it has heca's connection) names the session.
    receive(
        Cursor::new(json!({"session_id": "abc-1"}).to_string()),
        "example",
        "Stop",
        &reports,
        &folder,
    )
    .unwrap();
    fs::write(&endpoint, []).unwrap();
    // The daemon's hook has no connection, only the session id.
    receive_remembered(
        Cursor::new(json!({"session_id": "abc-1", "model": "m"}).to_string()),
        "example",
        "Stop",
        4096,
        &folder,
    )
    .unwrap();
    let report: AgentState = serde_json::from_slice(&fs::read(&endpoint).unwrap()).unwrap();
    assert_eq!(report.status.as_str(), "finished");
    assert_eq!(report.session_id.as_deref(), Some("abc-1"));
    // Another session, or another driver, was never seen: nothing is written.
    fs::write(&endpoint, []).unwrap();
    receive_remembered(
        Cursor::new(json!({"session_id": "other"}).to_string()),
        "example",
        "Stop",
        4096,
        &folder,
    )
    .unwrap();
    receive_remembered(
        Cursor::new(json!({"session_id": "abc-1"}).to_string()),
        "foreign",
        "Stop",
        4096,
        &folder,
    )
    .unwrap();
    assert!(fs::read(&endpoint).unwrap().is_empty());
    // A path in the id is not a file name.
    receive(
        Cursor::new(json!({"session_id": "../escape"}).to_string()),
        "example",
        "Stop",
        &reports,
        &folder,
    )
    .unwrap();
    assert!(
        !root.path().join("escape").exists() && !folder.join("example.../escape.json").exists()
    );
    // The pane closed: its file is gone, nothing is recreated, and the entry is dropped.
    fs::remove_file(&endpoint).unwrap();
    receive_remembered(
        Cursor::new(json!({"session_id": "abc-1"}).to_string()),
        "example",
        "Stop",
        4096,
        &folder,
    )
    .unwrap();
    assert!(!endpoint.exists());
    assert!(!folder.join("example.abc-1.json").exists());
}
