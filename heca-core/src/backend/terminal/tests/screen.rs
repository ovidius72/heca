//! The screen: its size, resizing, damage, input reaching the shell, wide characters.

use super::*;

#[test]
fn terminal_backend_initial_snapshot_matches_requested_size() {
    let _serial = shell_test_guard();
    let backend = TerminalBackend::new_for_test_with_shell(12, 5, TEST_SHELL)
        .expect("terminal backend should initialize");

    let snapshot = backend
        .terminal_snapshot()
        .expect("terminal backend should expose a snapshot");
    assert_eq!(snapshot.cols, 12);
    assert_eq!(snapshot.rows, 5);
    assert_eq!(snapshot.lines.len(), 5);
    assert!(snapshot.lines.iter().all(|line| line.cells.len() == 12));
}

#[test]
fn terminal_backend_resize_updates_snapshot_dimensions() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(12, 5, TEST_SHELL)
        .expect("terminal backend should initialize");

    backend.set_size(20, 8);

    let snapshot = backend
        .terminal_snapshot()
        .expect("terminal backend should expose a snapshot");
    assert_eq!(snapshot.cols, 20);
    assert_eq!(snapshot.rows, 8);
    assert_eq!(snapshot.lines.len(), 8);
    assert!(snapshot.lines.iter().all(|line| line.cells.len() == 20));
}

#[test]
fn terminal_backend_initial_damage_is_full_then_none() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(12, 5, TEST_SHELL)
        .expect("terminal backend should initialize");

    assert!(
        matches!(backend.take_terminal_damage(), TerminalDamage::Full),
        "initial terminal mount should force a full redraw"
    );
    assert!(
        matches!(backend.take_terminal_damage(), TerminalDamage::None),
        "without new terminal changes, damage should drain to None"
    );
}

#[test]
fn terminal_backend_resize_forces_full_damage() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(12, 5, TEST_SHELL)
        .expect("terminal backend should initialize");
    let _ = backend.take_terminal_damage();

    backend.set_size(20, 8);

    assert!(
        matches!(backend.take_terminal_damage(), TerminalDamage::Full),
        "resizing changes visible row identity and must force full redraw"
    );
}

#[test]
fn terminal_backend_process_input_reaches_shell() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
        .expect("terminal backend should initialize");
    let marker = "HECA_INPUT_OK";

    warm_shell(&mut backend);
    backend.process_input(shell_echo_command(marker).as_bytes());

    let saw_output = pump_backend_until(&mut backend, |backend| {
        terminal_text(backend).contains(marker)
    });

    assert!(saw_output, "shell output should include echoed marker");
}

#[test]
fn terminal_backend_output_produces_row_damage() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
        .expect("terminal backend should initialize");
    let marker = "HECA_DAMAGE_ROWS";

    warm_shell(&mut backend);
    let _ = backend.take_terminal_damage();
    backend.process_input(shell_echo_command(marker).as_bytes());

    let saw_output = pump_backend_until(&mut backend, |backend| {
        terminal_text(backend).contains(marker)
    });
    assert!(saw_output, "shell output should include echoed marker");

    match backend.take_terminal_damage() {
        TerminalDamage::Rows(ranges) => {
            assert!(
                !ranges.is_empty(),
                "ordinary output should produce at least one damaged visible row"
            );
        }
        other => panic!("expected row damage after ordinary output, got {other:?}"),
    }
}

#[test]
fn terminal_backend_preserves_grapheme_output() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
        .expect("terminal backend should initialize");
    let marker = "e\u{301}🙂";

    warm_shell(&mut backend);
    backend.process_input(shell_echo_command(marker).as_bytes());

    let saw_output = pump_backend_until(&mut backend, |backend| {
        terminal_text(backend).contains(marker)
    });

    assert!(
        saw_output,
        "terminal snapshot should preserve multi-codepoint grapheme output"
    );
}
