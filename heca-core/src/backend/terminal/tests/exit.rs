//! When the program exits: its code, and when the pane closes.

use super::*;

#[test]
fn terminal_backend_exit_sets_should_close() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
        .expect("terminal backend should initialize");

    warm_shell(&mut backend);
    backend.process_input(shell_exit_command().as_bytes());

    let exited = pump_backend_until(&mut backend, TerminalBackend::should_close);
    assert!(exited, "backend should report shell exit");
    assert!(
        backend.reaped,
        "backend should reap the PTY child after exit"
    );
}

#[test]
fn terminal_backend_exit_captures_code_via_take_exit() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
        .expect("terminal backend should initialize");

    warm_shell(&mut backend);
    backend.process_input(shell_exit_command().as_bytes());

    let exited = pump_backend_until(&mut backend, TerminalBackend::should_close);
    assert!(exited, "backend should report shell exit");

    // The exit code is captured (previously discarded — only
    // `is_some()` was checked) and drained once via `take_exit_code` so the
    // per-wake monitor can emit `pane.exited{code}`. `exit` ⇒ code 0.
    assert_eq!(backend.take_exit_code(), Some(0), "shell `exit` ⇒ code 0");
    assert_eq!(backend.take_exit_code(), None, "exit code drains once only");
    assert_eq!(
        backend.runtime().exit_code,
        Some(0),
        "runtime snapshot carries the captured exit code"
    );
}

#[test]
fn terminal_backend_update_is_stable_after_exit() {
    let _serial = shell_test_guard();
    let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
        .expect("terminal backend should initialize");

    warm_shell(&mut backend);
    backend.process_input(shell_exit_command().as_bytes());

    let exited = pump_backend_until(&mut backend, TerminalBackend::should_close);
    assert!(exited, "backend should report shell exit");
    assert!(
        backend.reaped,
        "backend should reap the PTY child before staying closed"
    );

    let _ = backend.update();
    let _ = backend.update();
    assert!(
        backend.should_close(),
        "backend should stay closed after repeated updates"
    );
    assert!(
        backend.reaped,
        "backend should remain reaped after repeated updates"
    );
}

#[test]
fn reconcile_exit_state_does_not_close_on_reader_disconnect_before_reap() {
    let mut exited = false;
    let mut reaped = false;

    reconcile_exit_state(true, &mut exited, &mut reaped, Ok(false));

    assert!(!exited, "reader disconnect alone should not close the pane");
    assert!(
        !reaped,
        "reader disconnect alone should not mark the child reaped"
    );
}

#[test]
fn reconcile_exit_state_closes_after_reap_when_reader_already_disconnected() {
    let mut exited = false;
    let mut reaped = false;

    reconcile_exit_state(true, &mut exited, &mut reaped, Ok(true));

    assert!(
        exited,
        "reaped child should close the pane once the reader is disconnected"
    );
    assert!(reaped, "reaped child should stay marked as reaped");
}

/// **A failed reap must not destroy a live pane**.
///
/// This asserted the opposite once, on the grounds that closing was the
/// "conservative" reading. It is not: the two mistakes cost different things. Closing a pane
/// that is still running throws away everything in it; leaving one open for another wake costs
/// nothing, because the next wake reaps it properly.
///
/// The bug it produced: a mouse divider drag resizes the PTY on every frame, and a pane would
/// vanish mid-drag after a few attempts — one unlucky `try_wait` was enough. The keyboard
/// resize, which fires a handful of times instead of hundreds, never did it.
#[test]
fn reconcile_exit_state_keeps_a_live_pane_when_the_reap_fails() {
    let mut exited = false;
    let mut reaped = false;

    reconcile_exit_state(
        false,
        &mut exited,
        &mut reaped,
        Err(io::Error::other("wait failed")),
    );

    assert!(
        !exited,
        "a failed reap means 'unknown', not 'exited' — closing here destroys a live pane",
    );
    assert!(
        !reaped,
        "…and it must stay reapable, or the real exit is never noticed",
    );
}

/// The safety net the change above must not remove: a child that cannot be reaped **and** whose
/// reader is gone is genuinely unreachable, so the pane closes instead of lingering forever.
#[test]
fn reconcile_exit_state_closes_when_the_reap_fails_and_the_reader_is_gone() {
    let mut exited = false;
    let mut reaped = false;

    reconcile_exit_state(
        true,
        &mut exited,
        &mut reaped,
        Err(io::Error::other("wait failed")),
    );

    assert!(
        exited,
        "nothing to read and nothing to reap — the child is gone"
    );
    assert!(reaped, "…so stop polling for it");
}
