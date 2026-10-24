//! The public contract, a real PTY and Heca's private helper, without a GUI or personal settings.

#![cfg(unix)]

use std::{path::PathBuf, sync::{Arc, mpsc}, time::{Duration, Instant}};

use heca::agent::{AgentDriver, AgentError, AgentInput, AgentIntegration, AgentLaunchContext, AgentOptions, AgentSetup, AgentState, AgentStatus, IntegrationContext, IntegrationResource};
use heca::agent::integration::{Integrations, InstallationState, IntegrationOperation};
use heca_core::{agent::AgentDrivers, backend::{PaneBackend, TerminalBackend, TerminalBackendOptions}, runtime::ProcessStatus};

struct ExampleDriver { programs: Vec<String> }

const INSTALLED_SCRIPT: &str = "[ -n \"$EXAMPLE_REPORT\" ] || exit 0\n\
    printf '%s\\n' \"$EXAMPLE_STATE_0\" >> \"$EXAMPLE_REPORT\"\nread step\n\
    printf '%s\\n' \"$EXAMPLE_STATE_1\" >> \"$EXAMPLE_REPORT\"\nread step\n\
    printf '%s\\n' \"$EXAMPLE_STATE_2\" >> \"$EXAMPLE_REPORT\"\nread step\n";

impl AgentDriver for ExampleDriver {
    fn id(&self) -> &str { "example" }
    fn name(&self) -> &str { "Example agent" }
    fn programs(&self) -> &[String] { &self.programs }
    fn integration(&self, context: &IntegrationContext) -> Result<Option<AgentIntegration>, AgentError> {
        Ok(Some(AgentIntegration {
            version: "1".into(),
            resources: vec![IntegrationResource::File {
                path: context.home.join("installed.sh"), content: INSTALLED_SCRIPT.as_bytes().to_vec(),
            }],
        }))
    }
    fn prepare(&self, context: &AgentLaunchContext) -> Result<AgentSetup, AgentError> {
        let mut setup = AgentSetup::default();
        setup.environment.insert("EXAMPLE_REPORT".into(), context.endpoint.as_ref().unwrap().to_string_lossy().into_owned());
        for (index, status) in ["working", "waiting_input", "finished"].iter().enumerate() {
            let mut report = AgentState::new(self.id(), self.name(), AgentStatus::new(*status));
            report.task = Some("Example task".into());
            report.model = Some("Example model".into());
            report.session_id = Some("example-session".into());
            report.session_title = Some("Example session".into());
            setup.environment.insert(format!("EXAMPLE_STATE_{index}"), serde_json::to_string(&report).unwrap());
        }
        Ok(setup)
    }
    fn read_status(&self, input: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> {
        let AgentInput::SideChannel(bytes) = input else { return Ok(None) };
        serde_json::from_slice(bytes).map(Some).map_err(|error| AgentError::Report(error.to_string()))
    }
}

#[test]
fn a_registered_program_reports_through_the_real_launch_and_pty_path() {
    let (_directory, mut backend, receiver) = terminal("printf '%s\\n' \"$EXAMPLE_STATE_0\" >> \"$EXAMPLE_REPORT\"\n\
        read step\nprintf '%s\\n' \"$EXAMPLE_STATE_1\" >> \"$EXAMPLE_REPORT\"\n\
        read step\nprintf '%s\\n' \"$EXAMPLE_STATE_2\" >> \"$EXAMPLE_REPORT\"\nread step\n");
    for status in ["working", "waiting_input", "finished"] {
        wait_for(&mut backend, &receiver, status);
        assert_eq!(backend.runtime().status, ProcessStatus::Running);
        assert_eq!(backend.runtime().agent.unwrap().task.as_deref(), Some("Example task"));
        backend.process_input(b"next\r");
    }
}

#[test]
fn reports_in_one_write_keep_each_transition_for_the_host() {
    let (_directory, mut backend, receiver) = terminal(
        "printf '%s\\n%s\\n%s\\n' \"$EXAMPLE_STATE_0\" \"$EXAMPLE_STATE_1\" \"$EXAMPLE_STATE_2\" >> \"$EXAMPLE_REPORT\"\nread step\n",
    );
    wait_for(&mut backend, &receiver, "finished");
    let statuses: Vec<_> = backend.take_agent_updates().into_iter().flatten().map(|report| report.status).collect();
    assert_eq!(statuses, ["working", "waiting_input", "finished"].map(AgentStatus::new));
    assert!(backend.take_agent_updates().is_empty(), "transitions are delivered once");
}

#[test]
fn an_installed_integration_reports_from_a_heca_launched_command() {
    check_installed_launch(false);
}

#[test]
fn an_installed_integration_reports_from_a_command_typed_in_an_existing_terminal() {
    check_installed_launch(true);
}

#[test]
fn an_installed_integration_stays_inactive_outside_heca() {
    let directory = tempfile::tempdir().unwrap();
    let installer = installer(directory.path(), example_drivers());
    installer.apply("example", IntegrationOperation::Install).unwrap();
    let output = std::process::Command::new("/bin/sh").arg(directory.path().join("installed.sh"))
        .env_clear().output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(installer.inspect("example").unwrap().state, InstallationState::Installed);
}

fn check_installed_launch(typed: bool) {
    let directory = tempfile::tempdir().unwrap();
    let drivers = example_drivers();
    let installer = installer(directory.path(), drivers.clone());
    installer.apply("example", IntegrationOperation::Install).unwrap();
    let command = format!("sh '{}'", directory.path().join("installed.sh").to_string_lossy().replace('\'', "'\\''"));
    let (mut backend, receiver) = start_terminal(directory.path(),
        if typed { "exec /bin/sh -i" } else { &command }, drivers);
    if typed { backend.process_input(format!("{command}\r").as_bytes()); }
    for status in ["working", "waiting_input", "finished"] {
        wait_for(&mut backend, &receiver, status);
        let report = backend.runtime().agent.unwrap();
        assert_eq!(report.model.as_deref(), Some("Example model"));
        assert_eq!(report.session_id.as_deref(), Some("example-session"));
        assert_eq!(report.session_title.as_deref(), Some("Example session"));
        backend.process_input(b"next\r");
    }
    drop(backend);
    assert_eq!(installer.inspect("example").unwrap().state, InstallationState::Installed,
        "closing the terminal must retain the persistent integration");
}

fn installer(directory: &std::path::Path, drivers: AgentDrivers) -> Integrations {
    Integrations::new(drivers, IntegrationContext {
        reporter: heca_core::agent::reporting::ReportingHelper { executable: std::env::current_exe().unwrap(), max_report_bytes: heca_config::settings::AgentsConfig::default().max_report_bytes },
        home: directory.into(), directory: directory.join("heca-data"), environment: Default::default(),
    }, directory.join("heca-config.toml"))
}

fn example_drivers() -> AgentDrivers {
    let mut drivers = AgentDrivers::new();
    drivers.register(ExampleDriver { programs: vec!["sh".into()] }).unwrap();
    drivers
}

fn terminal(script_text: &str) -> (tempfile::TempDir, TerminalBackend, mpsc::Receiver<()>) {
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("example.sh");
    std::fs::write(&script, script_text).unwrap();
    let command = format!("sh '{}'", script.to_string_lossy().replace('\'', "'\\''"));
    let (backend, receiver) = start_terminal(directory.path(), &command, example_drivers());
    (directory, backend, receiver)
}

fn start_terminal(directory: &std::path::Path, command: &str, drivers: AgentDrivers) -> (TerminalBackend, mpsc::Receiver<()>) {
    let (sender, receiver) = mpsc::channel();
    let wake = Arc::new(move || { let _ = sender.send(()); });
    let options = TerminalBackendOptions {
        agents: Some(AgentOptions {
            drivers, max_report_bytes: 65536, channel_capacity: 16,
            launch_timeout: Duration::from_secs(10), launcher: PathBuf::from(env!("CARGO_BIN_EXE_heca")),
        }),
        wake_on_output: Some(wake), shell_override: Some("/bin/sh".into()),
        cwd: Some(directory.into()), ..Default::default()
    };
    let backend = TerminalBackend::with_command(80, 24, 8.0, 16.0, command, options).unwrap();
    (backend, receiver)
}

fn wait_for(backend: &mut TerminalBackend, receiver: &mpsc::Receiver<()>, status: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        backend.update();
        if backend.runtime().agent.as_ref().is_some_and(|agent| agent.status.as_str() == status) { break }
        receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|error| panic!("waiting for {status}: {error}; runtime={:?}", backend.runtime()));
    }
}
