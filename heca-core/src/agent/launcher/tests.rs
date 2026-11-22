use std::{
    fs::OpenOptions,
    io::Write,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use super::*;
use crate::agent::{
    AgentDrivers, AgentOptions, AgentSession, AgentState, AgentStatus, tests::ExternalDriver,
};

#[test]
fn a_launch_uses_the_commands_current_environment_then_reports_without_terminal_output() {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExternalDriver::new("demo")).unwrap();
    let (sender, receiver) = mpsc::channel();
    let wake = Arc::new(move || {
        let _ = sender.send(());
    });
    let launcher = std::env::current_exe()
        .unwrap()
        .with_file_name("custom-host");
    let owner = AgentSession::prepare(
        AgentOptions {
            drivers,
            max_report_bytes: 10240,
            channel_capacity: 8,
            launch_timeout: Duration::from_secs(5),
            launcher: launcher.clone(),
            launched_status: String::new(),
        },
        Some(wake),
    )
    .unwrap();
    let root = owner.bin().parent().unwrap().to_owned();
    let run = tempfile::Builder::new()
        .prefix("run-")
        .tempdir_in(&root)
        .unwrap();
    let request = LaunchRequest {
        executable: "/opt/tools/demo-agent".into(),
        arguments: vec!["--label".into(), "two words".into()],
        program: "demo-agent".into(),
        cwd: run.path().into(),
        environment: [("DEMO_CONFIG".into(), "changed after the pane opened".into())].into(),
    };
    let setup = client::request_setup(&run, &request, Duration::from_secs(5), 10240).unwrap();
    assert_eq!(
        setup.environment["DEMO_CONFIG"],
        "changed after the pane opened"
    );
    assert_eq!(
        setup.environment["DEMO_LAUNCHER"],
        launcher.display().to_string()
    );
    assert!(
        !request_path(run.path()).unwrap().exists(),
        "the shell environment must not remain on disk"
    );
    let mut file = OpenOptions::new()
        .append(true)
        .open(run.path().join("reports.jsonl"))
        .unwrap();
    file.write_all(b"demo:working\n").unwrap();
    // A reporter appends and closes; Windows tells a watcher about the append when the file closes.
    drop(file);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut reports = Vec::new();
    while reports.is_empty() {
        receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        reports.extend(owner.drain());
    }
    let mut expected = AgentState::new("demo", "Demo", AgentStatus::new("working"));
    expected.launch = Some(crate::backend::ProcessLaunch {
        executable: request.executable,
        arguments: request.arguments,
        cwd: Some(request.cwd),
    });
    assert_eq!(reports, [expected]);
}

/// **An agent has its line from the moment it starts**, before it reports anything itself — the
/// status `launched_status` names, stamped by its driver and silent (heca's own event). OpenCode and
/// Codex say nothing until a turn starts; without this their panes showed no agent at all.
#[test]
fn a_started_agent_is_reported_before_its_own_first_report() {
    let started = |launched_status: &str, arguments: Vec<std::ffi::OsString>| {
        let mut drivers = AgentDrivers::new();
        drivers.register(ExternalDriver::new("demo")).unwrap();
        let (sender, receiver) = mpsc::channel();
        let wake = Arc::new(move || {
            let _ = sender.send(());
        });
        let owner = AgentSession::prepare(
            AgentOptions {
                drivers,
                max_report_bytes: 10240,
                channel_capacity: 8,
                launch_timeout: Duration::from_secs(5),
                launcher: std::env::current_exe().unwrap(),
                launched_status: launched_status.into(),
            },
            Some(wake),
        )
        .unwrap();
        let root = owner.bin().parent().unwrap().to_owned();
        let run = tempfile::Builder::new()
            .prefix("run-")
            .tempdir_in(&root)
            .unwrap();
        let request = LaunchRequest {
            executable: "/opt/tools/demo-agent".into(),
            arguments,
            program: "demo-agent".into(),
            cwd: run.path().into(),
            environment: Default::default(),
        };
        client::request_setup(&run, &request, Duration::from_secs(5), 10240).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut reports = Vec::new();
        while reports.is_empty() && Instant::now() < deadline {
            let _ = receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()));
            reports.extend(owner.drain());
        }
        reports
    };
    let reports = started("waiting_input", Vec::new());
    let first = reports
        .first()
        .expect("reported at launch, with nothing written by the agent");
    assert_eq!(first.driver, "demo");
    assert_eq!(first.status.as_str(), "waiting_input");
    assert_eq!(
        first.event.as_deref(),
        Some(super::super::channel::LAUNCHED_EVENT)
    );
    assert!(first.launch.is_some(), "it carries what was launched");
    assert_eq!(
        first.session_id, None,
        "a fresh start is in no conversation yet"
    );
    let resumed = started("waiting_input", vec!["--resume".into(), "conv-9".into()]);
    assert_eq!(
        resumed.first().and_then(|r| r.session_id.as_deref()),
        Some("conv-9"),
        "a resumed program keeps its conversation from launch"
    );
    assert!(
        started("", Vec::new()).is_empty(),
        "an empty launched_status reports nothing until the agent does"
    );
}
