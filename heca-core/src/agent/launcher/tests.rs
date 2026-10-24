use std::{fs::OpenOptions, io::Write, sync::{Arc, mpsc}, time::{Duration, Instant}};

use super::*;
use crate::agent::{AgentDrivers, AgentOptions, AgentSession, AgentState, AgentStatus, tests::ExternalDriver};

#[test]
fn a_launch_uses_the_commands_current_environment_then_reports_without_terminal_output() {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    let (sender, receiver) = mpsc::channel();
    let wake = Arc::new(move || { let _ = sender.send(()); });
    let launcher = std::env::current_exe().unwrap().with_file_name("custom-host");
    let owner = AgentSession::prepare(AgentOptions {
        drivers, max_report_bytes: 10240, channel_capacity: 8,
        launch_timeout: Duration::from_secs(5), launcher: launcher.clone(),
    }, Some(wake)).unwrap();
    let root = owner.bin().parent().unwrap().to_owned();
    let run = tempfile::Builder::new().prefix("run-").tempdir_in(&root).unwrap();
    let request = LaunchRequest {
        program: "demo-agent".into(), cwd: run.path().into(),
        environment: [("DEMO_CONFIG".into(), "changed after the pane opened".into())].into(),
    };
    let setup = client::request_setup(&run, &request, Duration::from_secs(5), 10240).unwrap();
    assert_eq!(setup.environment["DEMO_CONFIG"], "changed after the pane opened");
    assert_eq!(setup.environment["DEMO_LAUNCHER"], launcher.display().to_string());
    assert!(!request_path(run.path()).unwrap().exists(), "the shell environment must not remain on disk");
    let mut file = OpenOptions::new().append(true).open(run.path().join("reports.jsonl")).unwrap();
    file.write_all(b"demo:working\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut reports = Vec::new();
    while reports.is_empty() {
        receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
        reports.extend(owner.drain());
    }
    assert_eq!(reports, [AgentState::new("demo", "Demo", AgentStatus::new("working"))]);
}
