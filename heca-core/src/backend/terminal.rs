//! Terminal backend using `portable-pty` for process management and
//! `wezterm-term` for terminal emulation.

mod engine;
mod osc;
mod process;
mod pty;

use super::{
    BackendAlert, BackendKeyEvent, BackendMouseEvent, BackendRenderData, PaneBackend, PaneType,
    TerminalDamage, TerminalPaletteDefaults, TerminalSnapshot,
};
use crate::runtime::{ContentKind, PaneRuntime, ProcessStatus};
use engine::TerminalEngine;
use osc::{OscEvent, OscSnooper};
pub use pty::PtyError;
use pty::{PtyHandle, PtyRead};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Materialized shell-integration assets used to wrap interactive shells with
/// OSC 133 / OSC 7 hooks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellIntegrationAssets {
    /// Bash init file passed via `--init-file`.
    pub bash_init: PathBuf,
    /// Fish init snippet sourced via `fish -C`.
    pub fish_init: PathBuf,
    /// Zsh `ZDOTDIR` containing the injected `.zshrc`.
    pub zsh_zdotdir: PathBuf,
}

struct ShellLaunch<'a> {
    integration: Option<ShellIntegrationAssets>,
    override_path: Option<&'a str>,
}

/// Optional terminal-backend spawn behavior layered on top of the required
/// grid size and cell metrics.
#[derive(Default)]
pub struct TerminalBackendOptions {
    /// Terminal palette defaults injected into the emulation engine.
    pub palette_defaults: Option<TerminalPaletteDefaults>,
    /// Wake callback fired when PTY output arrives.
    pub wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional shell wrapper assets that inject OSC shell integration.
    pub shell_integration: Option<ShellIntegrationAssets>,
}

/// A backend that runs a real shell inside a PTY and models its state via
/// `wezterm-term`.
pub struct TerminalBackend {
    engine: TerminalEngine,
    pty: PtyHandle,
    cols: usize,
    rows: usize,
    cell_w: f32,
    cell_h: f32,
    reader_disconnected: bool,
    exited: bool,
    reaped: bool,
    dirty: bool,
    /// Cached canonical runtime truth (program/status/cwd/exit_code/kind). Updated
    /// in [`update`] (event-driven) and exposed via [`PaneBackend::runtime`].
    runtime: PaneRuntime,
    /// One-shot exit code captured when the PTY child is reaped; drained by the
    /// per-wake monitor via [`PaneBackend::take_exit_code`] to emit `pane.exited`.
    pending_exit: Option<i32>,
    /// Timestamp of the last foreground re-sample (debounce on the output-wake path).
    last_fg_check: Instant,
    /// Passive pre-parse OSC snooper for semantic prompt / cwd sequences.
    osc: OscSnooper,
    /// When true, semantic OSC status (`Success`/`Error`) is active and must not
    /// be immediately overwritten by shell-foreground `Idle` refreshes.
    semantic_status_active: bool,
}

impl TerminalBackend {
    /// Spawn a new terminal with the given grid size.
    pub fn new(cols: usize, rows: usize) -> Result<Self, PtyError> {
        Self::with_cell_size(cols, rows, 8.4, 14.0)
    }

    pub fn with_cell_size(cols: usize, rows: usize, cell_w: f32, cell_h: f32) -> Result<Self, PtyError> {
        Self::with_cell_size_and_defaults_and_waker(cols, rows, cell_w, cell_h, None, None)
    }

    pub fn with_cell_size_and_waker(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, PtyError> {
        Self::with_cell_size_and_defaults_and_waker(cols, rows, cell_w, cell_h, None, wake_on_output)
    }

    pub fn with_cell_size_and_defaults_and_waker(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        palette_defaults: Option<TerminalPaletteDefaults>,
        wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, PtyError> {
        Self::with_options(
            cols,
            rows,
            cell_w,
            cell_h,
            TerminalBackendOptions {
                palette_defaults,
                wake_on_output,
                shell_integration: None,
            },
        )
    }

    pub fn with_options(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        options: TerminalBackendOptions,
    ) -> Result<Self, PtyError> {
        Self::with_test_shell(
            cols,
            rows,
            cell_w,
            cell_h,
            options.palette_defaults,
            options.wake_on_output,
            ShellLaunch {
                integration: options.shell_integration,
                override_path: None,
            },
        )
    }

    fn with_test_shell(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        palette_defaults: Option<TerminalPaletteDefaults>,
        wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
        shell: ShellLaunch<'_>,
    ) -> Result<Self, PtyError> {
        let pty = match shell.override_path {
            Some(shell_path) => {
                PtyHandle::new_with_shell(
                    cols,
                    rows,
                    wake_on_output,
                    shell.integration,
                    Some(shell_path),
                )?
            }
            None => PtyHandle::new(cols, rows, wake_on_output, shell.integration)?,
        };
        let engine = TerminalEngine::new(cols, rows, pty.writer(), palette_defaults)?;

        // Seed `last_fg_check` one debounce in the past so the very first `update`
        // wake re-samples the foreground immediately (no 250 ms blind start).
        let last_fg_check = Instant::now()
            .checked_sub(process::FOREGROUND_DEBOUNCE)
            .unwrap_or_else(Instant::now);
        Ok(Self {
            engine,
            pty,
            cols,
            rows,
            cell_w,
            cell_h,
            reader_disconnected: false,
            exited: false,
            reaped: false,
            dirty: true,
            runtime: PaneRuntime {
                kind: ContentKind::Terminal,
                ..PaneRuntime::default()
            },
            pending_exit: None,
            last_fg_check,
            osc: OscSnooper::default(),
            semantic_status_active: false,
        })
    }

    /// Re-sample the PTY's foreground process (event-driven; debounced in
    /// [`update`](PaneBackend::update)). Updates the cached `runtime`
    /// program/status/cwd. `program == None` from the detector ⇒ the shell is
    /// foreground ⇒ report the shell basename + `Idle`.
    fn refresh_foreground(&mut self) {
        #[cfg(unix)]
        {
            let Some(master_fd) = self.pty.as_raw_fd() else {
                return;
            };
            let shell_pgrp = self.pty.process_group_leader().unwrap_or(-1);
            let Some(info) = process::detect_foreground(master_fd, shell_pgrp) else {
                return;
            };
            let shell_basename = std::path::Path::new(self.pty.shell_path())
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            if !self.semantic_status_active {
                self.runtime.status = if info.program.is_some() {
                    ProcessStatus::Running
                } else {
                    ProcessStatus::Idle
                };
            }
            self.runtime.program = info.program.or(shell_basename);
            if let Some(cwd) = info.cwd {
                self.runtime.cwd = Some(cwd);
            }
        }
        // Non-Unix: no foreground detection (stub); runtime stays at its default.
        #[cfg(not(unix))]
        {
            let _ = &mut self.runtime;
        }
    }

    fn apply_osc_event(&mut self, event: OscEvent) -> bool {
        match event {
            OscEvent::PromptStart => true,
            OscEvent::CommandStart | OscEvent::PreExec => {
                self.semantic_status_active = false;
                self.runtime.exit_code = None;
                true
            }
            OscEvent::CommandFinished(code) => {
                self.runtime.status = if code == 0 {
                    ProcessStatus::Success
                } else {
                    ProcessStatus::Error
                };
                self.runtime.exit_code = Some(code);
                self.semantic_status_active = true;
                false
            }
            OscEvent::Cwd(cwd) => {
                self.runtime.cwd = Some(cwd);
                false
            }
        }
    }

    #[cfg(test)]
    fn new_for_test_with_shell(cols: usize, rows: usize, shell: &str) -> Result<Self, PtyError> {
        Self::with_test_shell(
            cols,
            rows,
            8.4,
            14.0,
            None,
            None,
            ShellLaunch {
                integration: None,
                override_path: Some(shell),
            },
        )
    }
}

impl PaneBackend for TerminalBackend {
    fn pane_type(&self) -> PaneType {
        PaneType::Terminal
    }

    fn title(&self) -> &str {
        self.engine.title()
    }

    fn set_size(&mut self, cols: usize, rows: usize) {
        if self.cols == cols && self.rows == rows {
            return;
        }

        self.cols = cols;
        self.rows = rows;
        self.engine.resize(cols, rows);
        self.dirty = true;

        if self.pty.resize(cols, rows).is_err() {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] warning: failed to resize PTY to {}x{}; terminal model resized anyway",
                cols, rows
            );
        }
    }

    fn process_input(&mut self, data: &[u8]) {
        let _ = self.pty.write_input(data);
    }

    fn process_key_event(&mut self, event: &BackendKeyEvent) -> bool {
        self.engine.process_key_event(event)
    }

    fn process_mouse_event(&mut self, event: &BackendMouseEvent) -> bool {
        self.engine.process_mouse_event(event)
    }

    fn focus_changed(&mut self, focused: bool) {
        self.engine.focus_changed(focused);
    }

    fn take_alerts(&mut self) -> Vec<BackendAlert> {
        self.engine.take_alerts()
    }

    fn set_cell_size(&mut self, cell_w: f32, cell_h: f32) {
        self.cell_w = cell_w;
        self.cell_h = cell_h;
        self.dirty = true;
    }

    fn update(&mut self) -> bool {
        let mut had_data = false;

        loop {
            match self.pty.try_read() {
                PtyRead::Data(bytes) => {
                    let mut force_fg_refresh = false;
                    for event in self.osc.observe(&bytes) {
                        force_fg_refresh |= self.apply_osc_event(event);
                    }
                    self.engine.advance_bytes(&bytes);
                    if force_fg_refresh && !self.exited {
                        self.last_fg_check = Instant::now();
                        self.refresh_foreground();
                    }
                    had_data = true;
                }
                PtyRead::Empty => break,
                PtyRead::Disconnected => {
                    self.reader_disconnected = true;
                    break;
                }
            }
        }

        // Exit detection (event: reader-EOF wake → try_wait). Capture the exit
        // code (previously discarded — only `is_some()` was checked) so the
        // per-wake monitor can emit `pane.exited{code}`. No polling loop.
        let was_reaped = self.reaped;
        let mut reap_result: io::Result<bool> = Ok(false);
        let mut exit_code: Option<i32> = None;
        if !self.reaped {
            match self.pty.try_wait() {
                Ok(Some(status)) => {
                    exit_code = Some(status.exit_code() as i32);
                    reap_result = Ok(true);
                }
                Ok(None) => reap_result = Ok(false),
                Err(_) => reap_result = Err(io::Error::other("try_wait")),
            }
        }
        reconcile_exit_state(
            self.reader_disconnected,
            &mut self.exited,
            &mut self.reaped,
            reap_result,
        );
        if !was_reaped
            && self.reaped
            && let Some(code) = exit_code
        {
            self.runtime.exit_code = Some(code);
            self.pending_exit = Some(code);
        }

        // Foreground detection — event-first, debounced (NO periodic timer per
        // plan §0.2). Re-sample on the output/EOF wake, at most once per
        // `FOREGROUND_DEBOUNCE`, and only while the shell is alive.
        if !self.exited {
            let now = Instant::now();
            if process::fg_re_sample_due(now.duration_since(self.last_fg_check)) {
                self.last_fg_check = now;
                self.refresh_foreground();
            }
        }

        if had_data {
            self.dirty = true;
        }

        had_data
    }

    fn terminal_snapshot(&self) -> Option<TerminalSnapshot> {
        let snapshot = self.engine.snapshot(self.cell_size());
        snapshot.debug_assert_valid();
        Some(snapshot)
    }

    fn take_terminal_damage(&mut self) -> TerminalDamage {
        if std::mem::replace(&mut self.dirty, false) {
            TerminalDamage::Full
        } else {
            TerminalDamage::None
        }
    }

    fn render_data(&self) -> BackendRenderData {
        let snapshot = self
            .terminal_snapshot()
            .unwrap_or_else(|| unreachable!("TerminalBackend always produces a terminal snapshot"));
        BackendRenderData::Terminal {
            lines: snapshot.lines,
            cursor_col: snapshot.cursor.col,
            cursor_row: snapshot.cursor.row,
            cell_w: snapshot.cell_w,
            cell_h: snapshot.cell_h,
        }
    }

    fn should_close(&self) -> bool {
        self.exited
    }

    fn cell_size(&self) -> (f32, f32) {
        (self.cell_w, self.cell_h)
    }

    fn runtime(&self) -> PaneRuntime {
        self.runtime.clone()
    }

    fn take_exit_code(&mut self) -> Option<i32> {
        self.pending_exit.take()
    }
}

fn reconcile_exit_state(
    reader_disconnected: bool,
    exited: &mut bool,
    reaped: &mut bool,
    reap_result: io::Result<bool>,
) {
    if !*reaped {
        match reap_result {
            Ok(true) | Err(_) => {
                *exited = true;
                *reaped = true;
            }
            Ok(false) => {}
        }
    }

    if reader_disconnected && *reaped {
        *exited = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::thread;
    use std::time::{Duration, Instant};

    const TEST_TIMEOUT: Duration = Duration::from_secs(3);
    const TEST_POLL_INTERVAL: Duration = Duration::from_millis(20);

    #[cfg(not(windows))]
    const TEST_SHELL: &str = "/bin/sh";
    #[cfg(windows)]
    const TEST_SHELL: &str = "cmd.exe";

    #[test]
    fn terminal_backend_initial_snapshot_matches_requested_size() {
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
    fn terminal_backend_process_input_reaches_shell() {
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
    fn terminal_backend_preserves_grapheme_output() {
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

    #[test]
    fn terminal_backend_exit_sets_should_close() {
        let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
            .expect("terminal backend should initialize");

        warm_shell(&mut backend);
        backend.process_input(shell_exit_command().as_bytes());

        let exited = pump_backend_until(&mut backend, TerminalBackend::should_close);
        assert!(exited, "backend should report shell exit");
        assert!(backend.reaped, "backend should reap the PTY child after exit");
    }

    #[test]
    fn terminal_backend_exit_captures_code_via_take_exit() {
        let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
            .expect("terminal backend should initialize");

        warm_shell(&mut backend);
        backend.process_input(shell_exit_command().as_bytes());

        let exited = pump_backend_until(&mut backend, TerminalBackend::should_close);
        assert!(exited, "backend should report shell exit");

        // Phase 2: the exit code is captured (previously discarded — only
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
        let mut backend = TerminalBackend::new_for_test_with_shell(80, 24, TEST_SHELL)
            .expect("terminal backend should initialize");

        warm_shell(&mut backend);
        backend.process_input(shell_exit_command().as_bytes());

        let exited = pump_backend_until(&mut backend, TerminalBackend::should_close);
        assert!(exited, "backend should report shell exit");
        assert!(backend.reaped, "backend should reap the PTY child before staying closed");

        let _ = backend.update();
        let _ = backend.update();
        assert!(backend.should_close(), "backend should stay closed after repeated updates");
        assert!(backend.reaped, "backend should remain reaped after repeated updates");
    }

    #[test]
    fn reconcile_exit_state_does_not_close_on_reader_disconnect_before_reap() {
        let mut exited = false;
        let mut reaped = false;

        reconcile_exit_state(true, &mut exited, &mut reaped, Ok(false));

        assert!(!exited, "reader disconnect alone should not close the pane");
        assert!(!reaped, "reader disconnect alone should not mark the child reaped");
    }

    #[test]
    fn reconcile_exit_state_closes_after_reap_when_reader_already_disconnected() {
        let mut exited = false;
        let mut reaped = false;

        reconcile_exit_state(true, &mut exited, &mut reaped, Ok(true));

        assert!(exited, "reaped child should close the pane once the reader is disconnected");
        assert!(reaped, "reaped child should stay marked as reaped");
    }

    #[test]
    fn reconcile_exit_state_marks_backend_closed_on_wait_error() {
        let mut exited = false;
        let mut reaped = false;

        reconcile_exit_state(
            false,
            &mut exited,
            &mut reaped,
            Err(io::Error::other("wait failed")),
        );

        assert!(exited, "failed wait should conservatively close the backend");
        assert!(reaped, "failed wait should stop future reap polling");
    }

    #[test]
    fn terminal_backend_nvim_tui_produces_non_default_background_cells() {
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
            snapshot.lines.iter().flat_map(|line| line.cells.iter()).any(|cell| {
                color_differs(cell.bg, snapshot.default_bg)
            })
        });

        assert!(
            saw_colored_background,
            "nvim TUI should paint at least some cells with non-default background colors"
        );
    }

    #[test]
    fn terminal_backend_bash_integration_reports_success_error_and_cwd() {
        let bash = PathBuf::from("/bin/bash");
        if !bash.exists() {
            return;
        }

        let integration = write_shell_integration_assets_for_test();
        let mut backend = TerminalBackend::with_test_shell(
            80,
            24,
            8.4,
            14.0,
            None,
            None,
            ShellLaunch {
                integration: Some(integration),
                override_path: Some(bash.to_str().expect("bash path should be valid utf-8")),
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
    }

    fn warm_shell(backend: &mut TerminalBackend) {
        let deadline = Instant::now() + Duration::from_millis(200);
        while Instant::now() < deadline {
            let _ = backend.update();
            thread::sleep(TEST_POLL_INTERVAL);
        }
    }

    fn pump_backend_until<F>(backend: &mut TerminalBackend, mut predicate: F) -> bool
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

    fn terminal_text(backend: &TerminalBackend) -> String {
        let snapshot = backend
            .terminal_snapshot()
            .expect("terminal backend should expose a snapshot");

        snapshot
            .lines
            .iter()
            .map(|line| line.cells.iter().map(|cell| cell.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn color_differs(a: [f32; 4], b: [f32; 4]) -> bool {
        (a[0] - b[0]).abs() > 0.001
            || (a[1] - b[1]).abs() > 0.001
            || (a[2] - b[2]).abs() > 0.001
            || (a[3] - b[3]).abs() > 0.001
    }

    fn command_exists(name: &str) -> bool {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {name} >/dev/null 2>&1"))
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    fn shell_echo_command(marker: &str) -> String {
        format!("printf '{marker}'\r")
    }

    #[cfg(windows)]
    fn shell_echo_command(marker: &str) -> String {
        format!("echo {marker}\r")
    }

    fn shell_exit_command() -> &'static str {
        "exit\r"
    }

    fn write_shell_integration_assets_for_test() -> ShellIntegrationAssets {
        let root = std::env::temp_dir().join(format!(
            "heca-shell-integration-test-{}",
            std::process::id()
        ));
        let zsh_dir = root.join("zsh");
        fs::create_dir_all(&zsh_dir).expect("temp shell integration dir");

        let bash_init = root.join("bash_init.sh");
        let fish_init = root.join("fish_init.fish");
        let zsh_rc = zsh_dir.join(".zshrc");

        fs::write(
            &bash_init,
            r#"heca__osc() {
  printf '\033]%s\a' "$1"
}

heca__emit_cwd() {
  local host="${HOSTNAME:-localhost}"
  heca__osc "7;file://$host${PWD:-/}"
}

heca__prompt_command() {
  local exit_status="${1:-0}"
  if [ "${HECA_HAVE_PREEXEC:-0}" -eq 1 ] 2>/dev/null; then
    heca__osc "133;D;$exit_status"
  fi
  heca__osc "133;A"
  heca__emit_cwd
  HECA_HAVE_PREEXEC=0
  HECA_LAST_COMMAND=""
  return "$exit_status"
}

heca__preexec() {
  [ "${HECA_IN_PROMPT_COMMAND:-0}" -eq 1 ] 2>/dev/null && return
  local cmd="${BASH_COMMAND:-}"
  [ -z "$cmd" ] && return
  [ "$cmd" = "${HECA_LAST_COMMAND:-}" ] && return
  HECA_LAST_COMMAND="$cmd"
  HECA_HAVE_PREEXEC=1
  heca__osc "133;B"
  heca__osc "133;C"
}

trap 'heca__preexec' DEBUG
PROMPT_COMMAND='__heca_status=$?; HECA_IN_PROMPT_COMMAND=1; heca__prompt_command "$__heca_status"; HECA_IN_PROMPT_COMMAND=0'
"#,
        )
        .expect("write bash init");
        fs::write(
            &fish_init,
            include_str!("../../../heca/assets/shell-integration/fish_init.fish"),
        )
        .expect("write fish init");
        fs::write(
            &zsh_rc,
            include_str!("../../../heca/assets/shell-integration/zsh/.zshrc"),
        )
        .expect("write zsh rc");

        ShellIntegrationAssets {
            bash_init,
            fish_init,
            zsh_zdotdir: zsh_dir,
        }
    }
}
