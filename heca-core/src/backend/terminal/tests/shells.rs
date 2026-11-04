//! Real shells and programs: nvim's colours, bash and zsh integration, a command staying open.

use super::*;

#[test]
fn terminal_backend_nvim_tui_produces_non_default_background_cells() {
    let _serial = shell_test_guard();
    if !command_exists("nvim") {
        return;
    }

    let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
        .expect("terminal backend should initialize");
    warm_shell(&mut backend);
    backend.process_input(b"nvim --clean +'set termguicolors' +'colorscheme blue'\n");

    let saw_colored_background = pump_backend_until(&mut backend, |backend| {
        let snapshot = backend
            .terminal_snapshot()
            .expect("terminal backend should expose a snapshot");
        snapshot
            .lines
            .iter()
            .flat_map(|line| line.cells.iter())
            .any(|cell| color_differs(cell.bg, snapshot.default_bg))
    });

    assert!(
        saw_colored_background,
        "nvim TUI should paint at least some cells with non-default background colors"
    );
}

#[test]
fn terminal_backend_bash_integration_reports_success_error_and_cwd() {
    let _serial = shell_test_guard();
    let bash = PathBuf::from("/bin/bash");
    if !bash.exists() {
        return;
    }

    let integration = write_shell_integration_assets_for_test();
    // Isolate the shell from the user's environment: a ~/.bashrc that loads
    // bash-preexec (or any framework trapping DEBUG / rewriting
    // PROMPT_COMMAND) clobbers our OSC 133 hooks so `D;0` for `true` never
    // arrives. Spawn with a clean HOME (no ~/.bashrc to source) + the
    // inherited PATH so the test is deterministic across machines.
    let clean_home =
        std::env::temp_dir().join(format!("heca-bash-test-home-{}", std::process::id()));
    std::fs::create_dir_all(&clean_home).expect("create clean HOME for bash test");
    let env = vec![
        (
            "HOME".to_string(),
            clean_home.to_string_lossy().into_owned(),
        ),
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_default(),
        ),
    ];
    let mut backend = TerminalBackend::with_test_shell(
        80,
        24,
        TerminalBackendOptions {
            shell_integration: Some(integration),
            ..TerminalBackendOptions::default()
        },
        ShellLaunch {
            override_path: Some(bash.to_str().expect("bash path should be valid utf-8")),
            env_clear: true,
            env,
        },
    )
    .expect("terminal backend should initialize");

    warm_shell(&mut backend);
    backend.process_input(b"false\r");
    assert!(
        pump_backend_until(&mut backend, |backend| {
            backend.runtime().status == ProcessStatus::Error
                && backend.runtime().exit_code == Some(1)
        }),
        "bash shell integration should report `false` as Error with exit code 1"
    );

    backend.process_input(b"true\r");
    assert!(
        pump_backend_until(&mut backend, |backend| {
            backend.runtime().status == ProcessStatus::Success
                && backend.runtime().exit_code == Some(0)
        }),
        "bash shell integration should report `true` as Success with exit code 0"
    );

    backend.process_input(b"cd /tmp\r");
    assert!(
        pump_backend_until(&mut backend, |backend| {
            backend.runtime().cwd.as_deref() == Some(std::path::Path::new("/tmp"))
        }),
        "bash shell integration should update cwd via OSC 7 after `cd /tmp`"
    );

    // Clean up the isolated HOME (best-effort; a panic above just leaks it
    // in /tmp, which the OS reaps).
    let _ = std::fs::remove_dir_all(&clean_home);
}

#[test]
fn terminal_backend_zsh_integration_detects_nvim_as_running_program() {
    let _serial = shell_test_guard();
    let zsh = PathBuf::from("/bin/zsh");
    if !zsh.exists() || !command_exists("nvim") {
        return;
    }

    let integration = write_shell_integration_assets_for_test();
    let mut backend = TerminalBackend::with_test_shell(
        80,
        24,
        TerminalBackendOptions {
            shell_integration: Some(integration),
            ..TerminalBackendOptions::default()
        },
        ShellLaunch {
            override_path: Some(zsh.to_str().expect("zsh path should be valid utf-8")),
            env_clear: false,
            env: Vec::new(),
        },
    )
    .expect("terminal backend should initialize");

    warm_shell(&mut backend);
    backend.process_input(b"nvim --clean\r");

    let saw_nvim = pump_backend_until(&mut backend, |backend| {
        backend.runtime().program.as_deref() == Some("nvim")
            && backend.runtime().status == ProcessStatus::Running
    });
    let runtime = backend.runtime();

    assert!(
        saw_nvim,
        "zsh shell integration should report `nvim` as the running foreground program; got program={:?} status={:?} exit_code={:?}",
        runtime.program, runtime.status, runtime.exit_code,
    );

    let stable_until = Instant::now() + Duration::from_millis(400);
    let mut reverted = false;
    while Instant::now() < stable_until {
        let _ = backend.update();
        let runtime = backend.runtime();
        if runtime.program.as_deref() != Some("nvim")
            || runtime.status != ProcessStatus::Running
        {
            reverted = true;
            break;
        }
        thread::sleep(TEST_POLL_INTERVAL);
    }
    assert!(
        !reverted,
        "foreground program should remain `nvim / Running` while the command owns the PTY"
    );
}

#[test]
fn terminal_backend_command_spawn_runs_and_stays_open_for_policy() {
    // Spawns a real shell on a PTY, so it contends for the same scarce resource as the other
    // shell tests and waits its turn like them.
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::with_command(
        80,
        24,
        DEFAULT_CELL_SIZE.0,
        DEFAULT_CELL_SIZE.1,
        "printf 'phase6-ok\\n'; exit 7",
        TerminalBackendOptions {
            // **A test may not run the developer's shell.** A command is spawned as
            // `<shell> -ic`, so with `$SHELL` this ran the user's interactive rc: an
            // `oh-my-zsh` "Would you like to update? [Y/n]" prompt sat waiting for input that
            // never came, the command never reached `exit 7`, and this test hung for its full
            // 30s timeout — on one machine, and not on another.
            shell_override: Some("/bin/sh".to_string()),
            ..TerminalBackendOptions::default()
        },
    )
    .expect("command backend should initialize");

    assert!(
        pump_backend_until(&mut backend, |backend| backend.runtime().exit_code
            == Some(7)),
        "command backend should capture the exit code"
    );
    assert_eq!(backend.runtime().status, ProcessStatus::Error);
    assert_eq!(backend.take_alerts(), vec![crate::backend::BackendAlert::ProcessCompleted { exit_code: 7 }]);
    let _ = backend.update();
    assert!(backend.take_alerts().is_empty());
    assert!(
        terminal_text(&backend).contains("phase6-ok"),
        "command backend should render command output"
    );
    assert!(
        !backend.should_close(),
        "direct command backends stay open until pane close-policy decides"
    );
}
