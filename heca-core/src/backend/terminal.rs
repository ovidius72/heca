//! Terminal backend using `portable-pty` for process management and
//! `wezterm-term` for terminal emulation.

mod engine;
mod pty;

use super::{
    BackendKeyEvent, BackendMouseEvent, BackendRenderData, PaneBackend, PaneType, TerminalDamage,
    TerminalPaletteDefaults, TerminalSnapshot,
};
use engine::TerminalEngine;
pub use pty::PtyError;
use pty::{PtyHandle, PtyRead};
use std::io;
use std::sync::Arc;

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
        Self::with_test_shell(
            cols,
            rows,
            cell_w,
            cell_h,
            palette_defaults,
            wake_on_output,
            None,
        )
    }

    fn with_test_shell(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        palette_defaults: Option<TerminalPaletteDefaults>,
        wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
        shell_override: Option<&str>,
    ) -> Result<Self, PtyError> {
        let pty = match shell_override {
            Some(shell) => PtyHandle::new_with_shell(cols, rows, wake_on_output, Some(shell))?,
            None => PtyHandle::new(cols, rows, wake_on_output)?,
        };
        let engine = TerminalEngine::new(cols, rows, pty.writer(), palette_defaults)?;

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
        })
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
            Some(shell),
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
                    self.engine.advance_bytes(&bytes);
                    had_data = true;
                }
                PtyRead::Empty => break,
                PtyRead::Disconnected => {
                    self.reader_disconnected = true;
                    break;
                }
            }
        }

        let reap_result = if self.reaped {
            Ok(false)
        } else {
            self.pty.try_wait().map(|status| status.is_some())
        };
        reconcile_exit_state(
            self.reader_disconnected,
            &mut self.exited,
            &mut self.reaped,
            reap_result,
        );

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
}
