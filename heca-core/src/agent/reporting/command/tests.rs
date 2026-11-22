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

/// **A hook that does not name the model gets it from the conversation log** — the model of the last
/// answer, so a model changed mid-session (`/model`) shows at the next event. A hook that names it
/// keeps its own, and nothing else of the log crosses over.
#[test]
fn a_hook_without_the_model_reads_the_last_answers_model_from_the_log() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path());
    let log = root.path().join("transcript.jsonl");
    fs::write(
        &log,
        [
            json!({"type":"assistant","message":{"model":"model-old","content":"secret one"}}),
            json!({"type":"user","message":{"content":"private prompt"}}),
            json!({"type":"assistant","message":{"model":"model-new","content":"secret two"}}),
            json!({"type":"system","subtype":"note"}),
        ]
        .map(|line| line.to_string())
        .join("\n"),
    )
    .unwrap();
    let native = json!({"session_id":"s", "transcript_path": log.to_string_lossy()});
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
    assert_eq!(
        report.model.as_deref(),
        Some("model-new"),
        "the last answer's model"
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        !text.contains("secret") && !text.contains("private"),
        "{text}"
    );
}

/// **A model switch names the new model** (`to_model`, Claude Code's `PostModelSwitch`), which the
/// row shows at once.
#[test]
fn a_model_switch_reports_the_new_model() {
    let root = tempfile::tempdir().unwrap();
    let mut reports = reports(root.path());
    reports.events.insert(
        "PostModelSwitch".into(),
        AgentState::new("example", "Example", AgentStatus::new("waiting_input")),
    );
    let native = json!({"session_id":"s", "from_model":"model-old", "to_model":"model-new"});
    receive(
        Cursor::new(native.to_string()),
        "example",
        "PostModelSwitch",
        &reports,
        &root.path().join("sessions"),
    )
    .unwrap();
    let bytes = fs::read(reports.endpoint.as_ref().unwrap()).unwrap();
    let report: AgentState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report.model.as_deref(), Some("model-new"));
}

/// **At the end of a turn the answer may reach the log a moment after the hook runs**: while a typed
/// prompt follows the last answer, the end-of-turn hook looks again and reports the new answer's
/// model; any other hook does not wait.
#[test]
fn the_end_of_a_turn_waits_for_its_answer_to_reach_the_log() {
    use std::io::Write as _;
    let root = tempfile::tempdir().unwrap();
    let log = root.path().join("transcript.jsonl");
    fs::write(
        &log,
        format!(
            "{}\n{}\n",
            json!({"type":"assistant","message":{"model":"model-old"}}),
            json!({"type":"user","message":{"content":"ciao"}}),
        ),
    )
    .unwrap();
    assert_eq!(
        last_model(&log, false).as_deref(),
        Some("model-old"),
        "no wait outside a turn's end"
    );
    let late = log.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let mut file = fs::OpenOptions::new().append(true).open(late).unwrap();
        writeln!(
            file,
            "{}",
            json!({"type":"assistant","message":{"model":"model-new"}})
        )
        .unwrap();
    });
    assert_eq!(
        last_model(&log, true).as_deref(),
        Some("model-new"),
        "the turn's own answer"
    );
    writer.join().unwrap();
}
