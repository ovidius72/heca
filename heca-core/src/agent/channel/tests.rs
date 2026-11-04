use std::{fs::OpenOptions, io::Write, sync::Arc};

use super::*;
use crate::agent::{AgentStatus, tests::ExternalDriver};

#[test]
fn partial_reports_wait_for_a_newline_and_oversized_reports_do_not_leak() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reports");
    let mut reader = ReportReader::create(path.clone(), Arc::new(ExternalDriver::new("demo")), 16).unwrap();
    let inbox = Inbox::new(2);
    let mut writer = OpenOptions::new().append(true).open(path).unwrap();
    writer.write_all(b"demo:work").unwrap();
    reader.read(&inbox).unwrap();
    assert!(inbox.drain().is_empty());
    writer.write_all(b"ing\ndemo:this-record-exceeds-the-limit\ndemo:finished\n").unwrap();
    reader.read(&inbox).unwrap();
    let statuses: Vec<_> = inbox.drain().into_iter().map(|report| report.status).collect();
    assert_eq!(statuses, [AgentStatus::new("working"), AgentStatus::new("finished")]);
}

#[test]
fn a_full_inbox_keeps_the_latest_status() {
    let inbox = Inbox::new(1);
    for status in ["working", "waiting", "finished"] {
        inbox.push(AgentState::new("demo", "Demo", AgentStatus::new(status)));
    }
    assert_eq!(inbox.drain(), [AgentState::new("demo", "Demo", AgentStatus::new("finished"))]);
}

#[test]
fn a_coalesced_event_naming_another_child_still_processes_the_launch_queue() {
    let directory = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(directory.path()).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    fs::create_dir(root.join("run")).unwrap();
    fs::write(root.join("run.request.json"), serde_json::to_vec(&serde_json::json!({
        "program":"demo-agent", "cwd":root, "environment":{"DEMO_CONFIG":"current"},
        "executable":"/opt/tools/demo-agent", "arguments":[],
    })).unwrap()).unwrap();
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    // Supply only the coalesced event; no real root watcher or later event can mask a missed queue.
    let watcher = crate::agent::watcher::new(|_| {}).unwrap();
    let (sender, receiver) = mpsc::channel();
    sender.send(worker::Message::Event(Ok(notify::Event::new(
        notify::EventKind::Create(notify::event::CreateKind::Folder),
    ).add_path(root.join("bin"))))).unwrap();
    sender.send(worker::Message::Stop).unwrap();
    worker::run(watcher, receiver, root.clone(), drivers, crate::agent::reporting::ReportingHelper {
        executable: std::env::current_exe().unwrap(), max_report_bytes: 10240,
    }, Arc::new(Inbox::new(4)), None);
    let response: Result<crate::agent::AgentSetup, String> =
        serde_json::from_slice(&fs::read(root.join("run/response.json")).unwrap()).unwrap();
    assert_eq!(response.unwrap().environment["DEMO_CONFIG"], "current");
    assert!(!root.join("run.request.json").exists());
}
