//! Live opt-in must reach the terminal that was already open before project trust.

use super::*;
use crate::agent::{AgentDrivers, AgentOptions, TerminalNotification, tests::ExternalDriver};
use crate::backend::terminal::osc::OscEvent;

fn options(enabled: bool) -> AgentOptions {
    let mut drivers = AgentDrivers::new();
    if enabled { drivers.register(ExternalDriver::new("demo")).unwrap(); }
    AgentOptions { drivers, max_report_bytes: 1024, channel_capacity: 8,
        launch_timeout: Duration::from_secs(5), launcher: std::env::current_exe().unwrap() }
}

#[cfg(unix)]
#[test]
fn enabling_reporting_reaches_the_existing_terminal_and_replays_its_latest_status() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::with_command(80, 24, 8.0, 16.0, "cat", TerminalBackendOptions {
        agents: Some(options(false)), shell_override: Some(TEST_SHELL.into()), ..Default::default()
    }).unwrap();
    let process = backend.pty.process_group_leader();
    let bin = backend.agent_session.as_ref().unwrap().bin();
    backend.apply_osc_event(OscEvent::Program("demo-agent".into()));
    backend.apply_osc_event(OscEvent::Notification(TerminalNotification { code: 9, payload: "demo:working".into() }));
    assert!(backend.runtime().agent.is_none(), "disabled reporting remains silent");
    backend.reload_agent_config(Some(options(true))).unwrap();
    assert_eq!(backend.runtime().agent.unwrap().status.as_str(), "working");
    assert_eq!(backend.take_agent_updates().len(), 1, "one real transition reaches subscribers");
    assert_eq!(backend.pty.process_group_leader(), process, "the existing process stays alive");
    assert_eq!(backend.agent_session.as_ref().unwrap().bin(), bin);
    assert!(bin.join("demo-agent").exists(), "the existing shell's private PATH entry now resolves the enabled launcher");

    backend.reload_agent_config(Some(options(false))).unwrap();
    assert!(backend.runtime().agent.is_none());
    assert_eq!(backend.take_agent_updates(), [None]);
    assert!(!bin.join("demo-agent").exists(), "disabled integrations leave no launch wrapper");
    backend.apply_osc_event(OscEvent::CommandFinished(0));
    backend.reload_agent_config(Some(options(true))).unwrap();
    assert!(backend.runtime().agent.is_none(), "a finished command's report must not return on enable");

    backend.apply_osc_event(OscEvent::Program("demo-agent".into()));
    backend.apply_osc_event(OscEvent::Notification(TerminalNotification { code: 9, payload: "demo:working".into() }));
    backend.process_input(b"\x04");
    assert!(pump_backend_until(&mut backend, |backend| backend.exited));
    backend.reload_agent_config(Some(options(true))).unwrap();
    assert!(backend.runtime().agent.is_none(), "a reaped process cannot regain an agent status");
}
