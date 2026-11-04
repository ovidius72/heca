use super::*;
use crate::backend::BackendAlert;
use crate::backend::terminal::osc::OscEvent;

#[test]
fn command_completions_are_one_shot_events_not_runtime_snapshots() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(20, 6, TEST_SHELL).unwrap();
    backend.apply_osc_event(OscEvent::CommandFinished(0));
    assert!(backend.take_alerts().is_empty(), "an initial prompt is not a completion");
    for program in ["false", "vim", "arbitrary-program"] {
        backend.apply_osc_event(OscEvent::PreExec);
        backend.apply_osc_event(OscEvent::Program(program.into()));
        backend.apply_osc_event(OscEvent::CommandFinished(1));
        backend.apply_osc_event(OscEvent::CommandFinished(1));
        backend.apply_osc_event(OscEvent::Cwd(PathBuf::from("/tmp")));
    }
    assert_eq!(backend.take_alerts(), vec![BackendAlert::ProcessCompleted { exit_code: 1 }; 3]);
    assert!(backend.take_alerts().is_empty());
}

#[test]
fn bash_prompt_hooks_do_not_report_their_own_commands_or_completions() {
    let _serial = shell_test_guard();
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join(".bashrc"), "PROMPT_COMMAND='printf \"__OLD_PROMPT__\\n\"'\n").unwrap();
    let mut backend = TerminalBackend::with_test_shell(
        80, 24,
        TerminalBackendOptions {
            shell_integration: Some(write_shell_integration_assets_for_test()),
            ..Default::default()
        },
        ShellLaunch {
            override_path: Some("/bin/bash"),
            env_clear: true,
            env: vec![
                ("HOME".into(), home.path().to_string_lossy().into_owned()),
                ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
            ],
        },
    ).unwrap();
    warm_shell(&mut backend);
    assert!(pump_backend_until(&mut backend, |backend| backend.runtime().exit_code == Some(0)));
    backend.take_alerts();
    backend.process_input(b"false\r");
    assert!(pump_backend_until(&mut backend, |backend| backend.runtime().exit_code == Some(1)));
    assert_eq!(backend.runtime().status, ProcessStatus::Error);
    assert_eq!(backend.take_alerts(), vec![BackendAlert::ProcessCompleted { exit_code: 1 }]);
    let program = backend.runtime().program.unwrap();
    assert!(!program.starts_with("HECA_"), "internal prompt assignment leaked: {program}");
    assert!(!program.starts_with("heca__"), "internal prompt function leaked: {program}");
    assert!(terminal_text(&backend).contains("__OLD_PROMPT__"));
}
