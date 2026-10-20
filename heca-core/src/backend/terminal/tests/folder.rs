//! Starting in a given folder — and in a missing one.

use super::*;

/// A fresh, empty folder under the system temp dir, resolved (macOS's `/tmp` is a symlink, and
/// `pwd` reports the resolved path).
fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("heca-cwd-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir.canonicalize().expect("resolve scratch dir")
}

/// A shell starts in the folder it was told to start in.
#[test]
fn a_shell_starts_in_the_folder_it_is_given() {
    let _serial = shell_test_guard();
    let dir = scratch_dir("shell");
    let mut backend = TerminalBackend::with_test_shell(
        80,
        24,
        TerminalBackendOptions {
            cwd: Some(dir.clone()),
            ..TerminalBackendOptions::default()
        },
        ShellLaunch {
            override_path: Some("/bin/sh"),
            env_clear: false,
            env: Vec::new(),
        },
    )
    .expect("shell should start in an existing folder");
    warm_shell(&mut backend);
    backend.process_input(b"pwd\r");
    let want = dir.display().to_string();
    assert!(
        pump_backend_until(&mut backend, |b| terminal_text(b).contains(&want)),
        "the shell should report {want}, saw:\n{}",
        terminal_text(&backend)
    );
}

/// A spawned command starts in the folder it was told to start in.
#[test]
fn a_command_starts_in_the_folder_it_is_given() {
    let _serial = shell_test_guard();
    let dir = scratch_dir("command");
    let mut backend = TerminalBackend::with_command(
        80,
        24,
        DEFAULT_CELL_SIZE.0,
        DEFAULT_CELL_SIZE.1,
        "pwd; exit 0",
        TerminalBackendOptions {
            shell_override: Some("/bin/sh".to_string()),
            cwd: Some(dir.clone()),
            ..TerminalBackendOptions::default()
        },
    )
    .expect("command should start in an existing folder");
    let want = dir.display().to_string();
    assert!(
        pump_backend_until(&mut backend, |b| terminal_text(b).contains(&want)),
        "the command should report {want}, saw:\n{}",
        terminal_text(&backend)
    );
}

/// With no folder given, behaviour is what it always was: the process's own.
#[test]
fn no_folder_means_the_process_own() {
    assert!(TerminalBackendOptions::default().cwd.is_none());
}

/// A folder that does not exist is a failed spawn — the caller decides what to fall back to.
#[test]
fn a_missing_folder_is_a_failed_spawn() {
    let _serial = shell_test_guard();
    let missing = std::env::temp_dir().join("heca-cwd-does-not-exist-xyz");
    let result = TerminalBackend::with_command(
        80,
        24,
        DEFAULT_CELL_SIZE.0,
        DEFAULT_CELL_SIZE.1,
        "pwd",
        TerminalBackendOptions {
            shell_override: Some("/bin/sh".to_string()),
            cwd: Some(missing),
            ..TerminalBackendOptions::default()
        },
    );
    assert!(result.is_err(), "a missing folder must not spawn");
}
