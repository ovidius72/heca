use super::*;
use crate::backend::ProcessLaunch;

#[test]
fn an_original_process_launch_round_trips_and_replays_arguments_without_splitting() {
    let _guard = shell_test_guard();
    let folder = tempfile::tempdir().unwrap();
    let launch = ProcessLaunch {
        executable: TEST_SHELL.into(),
        arguments: vec![
            "-c".into(),
            "printf '%s' \"$1\"".into(),
            "heca-test".into(),
            "two words".into(),
        ],
        cwd: Some(folder.path().to_owned()),
    };
    let copy: ProcessLaunch =
        serde_json::from_slice(&serde_json::to_vec(&launch).unwrap()).unwrap();
    assert_eq!(copy, launch);
    let mut backend =
        TerminalBackend::with_process(80, 24, 8.0, 16.0, &copy, TerminalBackendOptions::default())
            .unwrap();
    assert!(pump_backend_until(&mut backend, |backend| backend.exited));
    assert_eq!(backend.process_launch(), Some(launch));
    let frame = backend.terminal_snapshot().unwrap();
    assert!(frame.lines.iter().any(|line| {
        line.cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<String>()
            .contains("two words")
    }));
}

#[test]
fn shell_reports_capture_the_current_command_and_drop_it_when_it_finishes() {
    use crate::backend::terminal::osc::OscEvent;
    let _guard = shell_test_guard();
    let folder = tempfile::tempdir().unwrap();
    let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL).unwrap();
    let shell = backend.process_launch().unwrap();
    backend.apply_osc_event(OscEvent::Cwd(folder.path().to_owned()));
    backend.apply_osc_event(OscEvent::CommandStart);
    backend.apply_osc_event(OscEvent::Program("editor".into()));
    backend.apply_osc_event(OscEvent::LaunchCommand(
        "editor 'file with spaces' --read-only".into(),
    ));
    let running = backend.process_launch().unwrap();
    assert_eq!(running.executable, shell.executable);
    assert_eq!(
        running.arguments.last().unwrap(),
        "editor 'file with spaces' --read-only"
    );
    assert_eq!(running.cwd.as_deref(), Some(folder.path()));
    backend.apply_osc_event(OscEvent::CommandFinished(0));
    assert_eq!(backend.process_launch(), Some(shell));
}

#[test]
fn a_real_bash_preexec_records_the_command_with_its_arguments_and_cwd() {
    let _guard = shell_test_guard();
    let folder = tempfile::tempdir().unwrap();
    let mut backend = TerminalBackend::with_test_shell(
        80,
        24,
        TerminalBackendOptions {
            shell_integration: Some(write_shell_integration_assets_for_test()),
            cwd: Some(folder.path().to_owned()),
            ..TerminalBackendOptions::default()
        },
        ShellLaunch {
            override_path: Some("/bin/bash"),
            env_clear: true,
            env: vec![
                ("HOME".into(), folder.path().display().to_string()),
                ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
            ],
        },
    )
    .unwrap();
    warm_shell(&mut backend);
    backend.process_input(b"sleep '2'\r");
    assert!(pump_backend_until(&mut backend, |backend| backend
        .process_launch()
        .is_some_and(|launch| launch
            .arguments
            .last()
            .is_some_and(|argument| argument == "sleep '2'"))));
    assert_eq!(
        backend.process_launch().unwrap().cwd.as_deref(),
        Some(folder.path().canonicalize().unwrap().as_path())
    );
}
