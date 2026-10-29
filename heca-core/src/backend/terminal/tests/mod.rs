//! The terminal backend's tests, by what they cover, and the helpers they share: starting a real
//! shell, waiting for it, and reading what it drew.

mod exit;
#[cfg(unix)]
mod agents;
mod folder;
mod screen;
mod shells;
mod feedback;
#[cfg(unix)]
mod recorded_launch;

// What the old single test module had in scope, named for the test files below.
pub(super) use super::launch::ShellLaunch;
pub(super) use super::program::reconcile_exit_state;
pub(super) use super::{ShellIntegrationAssets, TerminalBackend, TerminalBackendOptions};
pub(super) use crate::backend::{DEFAULT_CELL_SIZE, PaneBackend, TerminalDamage};
pub(super) use crate::runtime::ProcessStatus;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

// Generous on purpose: tests spawn real shells/TUIs (bash, nvim) whose
// first paint can lag under parallel-test CPU contention. This is only a SAFETY
// CAP, not a measured wait: `pump_backend_until` returns the instant its
// predicate is true (these tests usually finish well under a second), so a
// generous cap costs nothing on the happy path and just removes false timeouts
// under load. 3s flaked ~1 in 3 runs; 6s still flaked when tests ran alongside
// clippy (a real shell starved of CPU echoes after the deadline). 30s is ample.
pub(super) const TEST_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const TEST_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[cfg(not(windows))]
pub(super) const TEST_SHELL: &str = "/bin/sh";
#[cfg(windows)]
pub(super) const TEST_SHELL: &str = "cmd.exe";

/// Serialises the tests that spawn a real shell on a PTY.
///
/// These contend for a scarce OS resource: every one of them forks a shell and
/// allocates a pty, and `cargo test` runs them in parallel. Under that contention
/// a shell intermittently never came up at all — no output for the full 30s
/// timeout — landing on a different test each run. Running the suite with
/// `--test-threads=1` made it disappear completely (0 failures in 4 runs against
/// ~2 in 6 parallel), which is what identified the contention as the cause rather
/// than anything in the backend.
///
/// A test-local lock is used instead of demanding `--test-threads=1` globally, so
/// only these tests serialise and the other ~75 in this crate still run in
/// parallel.
pub(super) fn shell_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static SHELL_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A panicking test poisons the lock; that failure is already being reported,
    // so recover rather than cascading an unrelated PoisonError into every
    // sibling test.
    SHELL_TESTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// How long to wait for one handshake attempt before re-sending it.
pub(super) const WARM_ATTEMPT: Duration = Duration::from_millis(400);

/// Block until the shell is genuinely ready to *read* input, by round-tripping a
/// marker through it and re-sending until it comes back.
///
/// This was a flat 200ms sleep, and it made every test that types into the shell
/// intermittently fail — roughly two runs in six locally, landing on a different
/// test each time and burning the full 30s timeout when it did.
///
/// The cause is not merely that a shell can take longer than 200ms to start under
/// `cargo test`'s parallelism. It is that `sh` calls `tcsetattr(TCSAFLUSH)` while
/// setting up the terminal, and **`TCSAFLUSH` discards pending input**. Anything
/// written into the PTY before that moment is thrown away, not buffered — so
/// "write early and let the tty queue it" does not work, and no amount of waiting
/// *afterwards* recovers the lost keystrokes.
///
/// Waiting for the shell's *prompt* is not a fix either: whether `sh` prints one
/// depends on `PS1` and on how it judges interactivity, so a prompt may never
/// arrive. The only reliable readiness signal is a full round trip, retried until
/// it survives the flush.
pub(super) fn warm_shell(backend: &mut TerminalBackend) {
    const READY_MARKER: &str = "HECA_SHELL_READY";
    let deadline = Instant::now() + TEST_TIMEOUT;
    while Instant::now() < deadline {
        backend.process_input(shell_echo_command(READY_MARKER).as_bytes());
        let seen = pump_backend_for(backend, WARM_ATTEMPT, |backend| {
            terminal_text(backend).contains(READY_MARKER)
        });
        if seen {
            return;
        }
    }
    panic!("shell never echoed a handshake within {TEST_TIMEOUT:?}");
}

/// [`pump_backend_until`] with an explicit, shorter budget — for polling that is
/// expected to be retried rather than to succeed on the first attempt.
pub(super) fn pump_backend_for<F>(
    backend: &mut TerminalBackend,
    budget: Duration,
    mut predicate: F,
) -> bool
where
    F: FnMut(&TerminalBackend) -> bool,
{
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        let _ = backend.update();
        if predicate(backend) {
            return true;
        }
        thread::sleep(TEST_POLL_INTERVAL);
    }
    false
}

pub(super) fn pump_backend_until<F>(backend: &mut TerminalBackend, mut predicate: F) -> bool
where
    F: FnMut(&TerminalBackend) -> bool,
{
    let deadline = Instant::now() + TEST_TIMEOUT;
    while Instant::now() < deadline {
        let _ = backend.update();
        if predicate(backend) {
            return true;
        }
        thread::sleep(TEST_POLL_INTERVAL);
    }
    false
}

pub(super) fn terminal_text(backend: &TerminalBackend) -> String {
    let snapshot = backend
        .terminal_snapshot()
        .expect("terminal backend should expose a snapshot");

    snapshot
        .lines
        .iter()
        .map(|line| {
            line.cells
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn color_differs(a: [f32; 4], b: [f32; 4]) -> bool {
    (a[0] - b[0]).abs() > 0.001
        || (a[1] - b[1]).abs() > 0.001
        || (a[2] - b[2]).abs() > 0.001
        || (a[3] - b[3]).abs() > 0.001
}

pub(super) fn command_exists(name: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
pub(super) fn shell_echo_command(marker: &str) -> String {
    format!("printf '{marker}'\r")
}

#[cfg(windows)]
pub(super) fn shell_echo_command(marker: &str) -> String {
    format!("echo {marker}\r")
}

pub(super) fn shell_exit_command() -> &'static str {
    "exit\r"
}

/// Hands every caller its own shell-integration directory.
///
/// The path used to be keyed on the **process id** alone, so every test in the
/// binary shared one directory — while `cargo test` runs them in parallel and at
/// least one of them ends by `remove_dir_all`-ing its tree. A test could therefore
/// have its rcfiles deleted out from under a shell that was still starting, after
/// which that shell produced no output at all and the test spun out its full 30s
/// timeout. It presented as flakiness (~2 runs in 6, landing on a different test
/// each time) but it is a straightforward shared-mutable-state bug: serialising
/// the suite made it vanish, which is what pinned it.
///
/// A per-call counter is enough — the pid still separates concurrent `cargo test`
/// invocations, and the counter separates tests within one binary.
pub(super) fn write_shell_integration_assets_for_test() -> ShellIntegrationAssets {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "heca-shell-integration-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let zsh_dir = root.join("zsh");
    fs::create_dir_all(&zsh_dir).expect("temp shell integration dir");

    let bash_init = root.join("bash_init.sh");
    let fish_init = root.join("fish_init.fish");
    let zsh_rc = zsh_dir.join(".zshrc");

    fs::write(
        &bash_init,
        include_str!("../../../../../heca/assets/shell-integration/bash_init.sh"),
    )
    .expect("write bash init");
    fs::write(
        &fish_init,
        include_str!("../../../../../heca/assets/shell-integration/fish_init.fish"),
    )
    .expect("write fish init");
    fs::write(
        &zsh_rc,
        include_str!("../../../../../heca/assets/shell-integration/zsh/.zshrc"),
    )
    .expect("write zsh rc");

    ShellIntegrationAssets {
        bash_init,
        fish_init,
        zsh_zdotdir: zsh_dir,
    }
}
