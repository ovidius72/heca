//! Terminal backend using `portable-pty` for process management and
//! `wezterm-term` for terminal emulation.

mod engine;
mod osc;
mod process;
mod pty;

use super::{
    BackendAlert, BackendKeyEvent, BackendMouseEvent, BackendRenderData, PaneBackend, PaneType,
    TerminalDamage, TerminalLine, TerminalPaletteDefaults, TerminalRowRange, TerminalSnapshot,
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
    /// Clear the inherited environment before spawning, isolating the shell from
    /// the user's shell config (e.g. a `~/.bashrc` that loads bash-preexec and
    /// clobbers our OSC 133 `DEBUG` trap / `PROMPT_COMMAND`). `env` is applied
    /// after the clear. Default `false` — the real app keeps the user's env.
    env_clear: bool,
    /// Extra env vars to set on the spawned shell (after an optional
    /// `env_clear`). Used by integration tests to give the shell a clean `HOME`
    /// so the test is deterministic across machines.
    env: Vec<(String, String)>,
}

struct CommandLaunch<'a> {
    command: &'a str,
}

enum LaunchTarget<'a> {
    Shell(ShellLaunch<'a>),
    Command(CommandLaunch<'a>),
}

/// Optional terminal-backend spawn behavior layered on top of the required
/// grid size and cell metrics.
pub struct TerminalBackendOptions {
    /// Terminal palette defaults injected into the emulation engine.
    pub palette_defaults: Option<TerminalPaletteDefaults>,
    /// Wake callback fired when PTY output arrives.
    pub wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Optional shell wrapper assets that inject OSC shell integration.
    pub shell_integration: Option<ShellIntegrationAssets>,
    /// Host terminal scrollback capacity in rows (`wezterm-term` scrollback).
    /// `0` keeps wezterm-term's trait default (3500).
    pub scrollback_size: usize,
    /// Enable backend-side viewport easing for animated scroll APIs.
    pub scroll_animations: bool,
}

impl TerminalBackendOptions {
    /// Default host terminal scrollback capacity in rows. Mirrors wezterm-term's
    /// `TerminalConfiguration::scrollback_size()` default so behaviour is
    /// unchanged when the caller does not care.
    pub const DEFAULT_SCROLLBACK_SIZE: usize = 3500;
}

impl Default for TerminalBackendOptions {
    fn default() -> Self {
        Self {
            palette_defaults: None,
            wake_on_output: None,
            shell_integration: None,
            scrollback_size: Self::DEFAULT_SCROLLBACK_SIZE,
            scroll_animations: true,
        }
    }
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
    /// Device scale factor (logical→physical). Pixel dimensions reported to the
    /// emulation model + PTY are `cell × scale` so inline images are generated and
    /// sized at the real on-screen resolution (crisp on HiDPI). Defaults to 1.0
    /// until the app pushes the window scale via [`PaneBackend::set_scale_factor`].
    cell_scale: f32,
    reader_disconnected: bool,
    exited: bool,
    reaped: bool,
    force_full_damage: bool,
    last_damage_seqno: Option<usize>,
    last_damage_viewport_top: Option<isize>,
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
    /// Short event-first refresh window armed by shell preexec so the backend
    /// re-samples foreground ownership again after the shell actually hands the
    /// PTY to the child process. This avoids getting stuck on `shell/Idle` when
    /// a program starts between the immediate preexec sample and the next prompt.
    post_command_fg_refreshes: u8,
    /// When shell integration told us the foreground command basename, treat it
    /// as authoritative until the matching command finishes instead of letting
    /// later fallback foreground samples clobber it back to the shell.
    shell_reported_program_active: bool,
    /// Whether the pane should auto-close as soon as the PTY child exits.
    auto_close_on_exit: bool,
}

const POST_COMMAND_FG_REFRESHES: u8 = 2;

impl TerminalBackend {
    /// Spawn a new terminal with the given grid size.
    pub fn new(cols: usize, rows: usize) -> Result<Self, PtyError> {
        Self::with_cell_size(cols, rows, 8.4, 14.0)
    }

    pub fn with_cell_size(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
    ) -> Result<Self, PtyError> {
        Self::with_cell_size_and_defaults_and_waker(cols, rows, cell_w, cell_h, None, None)
    }

    pub fn with_cell_size_and_waker(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, PtyError> {
        Self::with_cell_size_and_defaults_and_waker(
            cols,
            rows,
            cell_w,
            cell_h,
            None,
            wake_on_output,
        )
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
                scrollback_size: TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
                scroll_animations: true,
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
        Self::with_launch_target(
            cols,
            rows,
            (cell_w, cell_h),
            options.palette_defaults,
            options.wake_on_output,
            options.scrollback_size,
            options.scroll_animations,
            LaunchTarget::Shell(ShellLaunch {
                integration: options.shell_integration,
                override_path: None,
                env_clear: false,
                env: Vec::new(),
            }),
        )
    }

    pub fn with_command(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        command: &str,
        options: TerminalBackendOptions,
    ) -> Result<Self, PtyError> {
        Self::with_launch_target(
            cols,
            rows,
            (cell_w, cell_h),
            options.palette_defaults,
            options.wake_on_output,
            options.scrollback_size,
            options.scroll_animations,
            LaunchTarget::Command(CommandLaunch { command }),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "terminal launch wiring threads PTY palette/wake/scrollback/animation policy explicitly; grouping is a later refactor"
    )]
    fn with_launch_target(
        cols: usize,
        rows: usize,
        cell_size: (f32, f32),
        palette_defaults: Option<TerminalPaletteDefaults>,
        wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
        scrollback_size: usize,
        scroll_animations: bool,
        launch: LaunchTarget<'_>,
    ) -> Result<Self, PtyError> {
        let (cell_w, cell_h) = cell_size;
        let (pty, auto_close_on_exit) = match launch {
            LaunchTarget::Shell(shell) => {
                let pty = match shell.override_path {
                    Some(shell_path) => {
                        PtyHandle::new_with_shell(
                            cols,
                            rows,
                            cell_size,
                            wake_on_output,
                            shell.integration,
                            Some(shell_path),
                            shell.env_clear,
                            &shell.env,
                        )?
                    }
                    None => {
                        PtyHandle::new(cols, rows, cell_size, wake_on_output, shell.integration)?
                    }
                };
                (pty, true)
            }
            LaunchTarget::Command(command) => (
                PtyHandle::new_with_command(
                    cols,
                    rows,
                    cell_size,
                    wake_on_output,
                    command.command,
                )?,
                false,
            ),
        };
        let mut engine = TerminalEngine::new(
            cols,
            rows,
            (cell_w, cell_h),
            pty.writer(),
            palette_defaults,
            scrollback_size,
        )?;
        engine.set_scroll_animations_enabled(scroll_animations);

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
            cell_scale: 1.0,
            reader_disconnected: false,
            exited: false,
            reaped: false,
            force_full_damage: true,
            last_damage_seqno: None,
            last_damage_viewport_top: None,
            runtime: PaneRuntime {
                kind: ContentKind::Terminal,
                ..PaneRuntime::default()
            },
            pending_exit: None,
            last_fg_check,
            osc: OscSnooper::default(),
            semantic_status_active: false,
            post_command_fg_refreshes: 0,
            shell_reported_program_active: false,
            auto_close_on_exit,
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
            if self.shell_reported_program_active {
                if let Some(cwd) = info.cwd {
                    self.runtime.cwd = Some(cwd);
                }
                return;
            }
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
            OscEvent::Program(program) => {
                if !program.is_empty() {
                    self.runtime.program = Some(program);
                    self.runtime.status = ProcessStatus::Running;
                    self.semantic_status_active = false;
                    self.runtime.exit_code = None;
                    self.shell_reported_program_active = true;
                }
                false
            }
            OscEvent::CommandFinished(code) => {
                self.runtime.status = if code == 0 {
                    ProcessStatus::Success
                } else {
                    ProcessStatus::Error
                };
                self.runtime.exit_code = Some(code);
                self.semantic_status_active = true;
                self.shell_reported_program_active = false;
                false
            }
            OscEvent::Cwd(cwd) => {
                self.runtime.cwd = Some(cwd);
                false
            }
        }
    }

    #[cfg(test)]
    fn with_test_shell(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        palette_defaults: Option<TerminalPaletteDefaults>,
        wake_on_output: Option<Arc<dyn Fn() + Send + Sync>>,
        shell: ShellLaunch<'_>,
    ) -> Result<Self, PtyError> {
        Self::with_launch_target(
            cols,
            rows,
            (cell_w, cell_h),
            palette_defaults,
            wake_on_output,
            TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
            true,
            LaunchTarget::Shell(shell),
        )
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
                env_clear: false,
                env: Vec::new(),
            },
        )
    }

    /// Physical cell pixel size `(cell × scale)` reported to the emulation model
    /// and the PTY, so inline images render at the real on-screen resolution.
    fn physical_cell_px(&self) -> (f32, f32) {
        (self.cell_w * self.cell_scale, self.cell_h * self.cell_scale)
    }

    /// Push the current physical cell pixel size to both reporters: the wezterm
    /// model (image sizing + `CSI 14/16 t` answers) and the PTY winsize
    /// (`TIOCGWINSZ`, read by image tools like Yazi / kitten).
    fn push_pixel_metrics(&mut self) {
        let px = self.physical_cell_px();
        self.engine.set_cell_px(px);
        let _ = self.pty.resize(self.cols, self.rows, px);
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
        self.force_full_damage = true;

        if self.pty.resize(cols, rows, self.physical_cell_px()).is_err() {
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

    fn take_clipboard_writes(&mut self) -> Vec<String> {
        self.engine.take_clipboard_writes()
    }

    fn paste(&mut self, text: &str) {
        // Wrap in bracketed-paste markers when the program enabled DECSET 2004, so
        // editors/shells treat the whole blob as literal pasted text (no auto-indent,
        // no executing newlines). Otherwise forward verbatim.
        if self.engine.bracketed_paste_enabled() {
            let mut framed = Vec::with_capacity(text.len() + 12);
            framed.extend_from_slice(b"\x1b[200~");
            framed.extend_from_slice(text.as_bytes());
            framed.extend_from_slice(b"\x1b[201~");
            self.process_input(&framed);
        } else {
            self.process_input(text.as_bytes());
        }
    }

    fn set_cell_size(&mut self, cell_w: f32, cell_h: f32) {
        self.cell_w = cell_w;
        self.cell_h = cell_h;
        self.push_pixel_metrics();
        self.force_full_damage = true;
    }

    fn set_scale_factor(&mut self, scale: f32) {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        if (self.cell_scale - scale).abs() < f32::EPSILON {
            return;
        }
        self.cell_scale = scale;
        self.push_pixel_metrics();
    }

    fn update(&mut self) -> bool {
        let mut had_data = false;
        let mut saw_program_event = false;

        loop {
            match self.pty.try_read() {
                PtyRead::Data(bytes) => {
                    let mut force_fg_refresh = false;
                    for event in self.osc.observe(&bytes) {
                        saw_program_event |= matches!(&event, OscEvent::Program(_));
                        force_fg_refresh |= self.apply_osc_event(event);
                    }
                    self.engine.advance_bytes(&bytes);
                    if force_fg_refresh && !self.exited {
                        self.post_command_fg_refreshes = POST_COMMAND_FG_REFRESHES;
                        if !saw_program_event {
                            self.last_fg_check = Instant::now();
                            self.refresh_foreground();
                        }
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
            self.runtime.status = if code == 0 {
                ProcessStatus::Success
            } else {
                ProcessStatus::Error
            };
            self.runtime.exit_code = Some(code);
            self.pending_exit = Some(code);
        }

        if had_data && self.post_command_fg_refreshes > 0 && !self.exited && !saw_program_event {
            self.last_fg_check = Instant::now();
            self.refresh_foreground();
            self.post_command_fg_refreshes -= 1;
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

        // Re-clamp the host viewport after draining output. PTY output is the
        // only path that shrinks scrollback without a resize (alt-screen entry,
        // `\x1b[2J` clears), so reconciling here keeps the stored `viewport_offset`
        // self-consistent before any downstream `snapshot()` read and surfaces the
        // correction as `TerminalDamage::Full` via `viewport_changed`.
        self.engine.reconcile_viewport_offset();

        // Snap-to-bottom policy (Q5): when new terminal output arrives and the
        // viewport is already at the live bottom (`at_bottom() == true` after
        // `reconcile_viewport_offset`), defensively re-snap so the growing
        // live region stays visible. The call is a no-op when already at offset 0
        // (the common case) but guards against future code paths that may move
        // the offset during reconciliation without marking the viewport dirty.
        // If the user has scrolled up, `at_bottom()` returns `false` and we do
        // NOT steal their viewport position — they'll snap back on key input.
        if had_data && self.engine.at_bottom() {
            self.engine.scroll_to_bottom();
        }

        had_data
    }

    fn terminal_snapshot(&self) -> Option<TerminalSnapshot> {
        let snapshot = self.engine.snapshot(self.cell_size());
        snapshot.debug_assert_valid();
        Some(snapshot)
    }

    fn take_terminal_damage(&mut self) -> TerminalDamage {
        let current_seqno = self.engine.current_seqno();
        let viewport_top = self.engine.visible_top_stable_row();
        // Q6: host viewport motion produces `TerminalDamage::Full` for the
        // retained terminal layer (incremental viewport damage is the separate
        // `terminal-01` phase). Drain the flag so only motion since the last read
        // arms the next frame.
        let viewport_changed = self.engine.take_viewport_changed();

        let damage = if self.force_full_damage || viewport_changed {
            TerminalDamage::Full
        } else if self.last_damage_seqno == Some(current_seqno)
            && self.last_damage_viewport_top == Some(viewport_top)
        {
            TerminalDamage::None
        } else if self.last_damage_viewport_top != Some(viewport_top) {
            // Viewport motion invalidates visible row identity, so per-row
            // damage is unsafe until retained-content work can reason about it.
            TerminalDamage::Full
        } else if let Some(last_seqno) = self.last_damage_seqno {
            coalesce_terminal_damage_rows(self.engine.changed_visible_rows_since(last_seqno))
        } else {
            TerminalDamage::Full
        };

        self.force_full_damage = false;
        self.last_damage_seqno = Some(current_seqno);
        self.last_damage_viewport_top = Some(viewport_top);
        damage
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
        self.exited && self.auto_close_on_exit
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

    fn is_mouse_grabbed(&self) -> bool {
        self.engine.is_mouse_grabbed()
    }

    fn at_bottom(&self) -> bool {
        self.engine.at_bottom()
    }

    fn scroll_viewport(&mut self, delta_rows: i32) {
        self.engine.scroll_viewport(delta_rows);
    }

    fn scroll_to_top(&mut self) {
        self.engine.scroll_to_top();
    }

    fn scroll_to_bottom(&mut self) {
        self.engine.scroll_to_bottom();
    }

    fn tick_animation(&mut self) -> bool {
        self.engine.advance_animation()
    }

    fn scroll_viewport_animated(&mut self, delta_rows: i32) {
        self.engine.scroll_viewport_animated(delta_rows);
    }

    fn scroll_to_top_animated(&mut self) {
        self.engine.scroll_to_top_animated();
    }

    fn scroll_to_bottom_animated(&mut self) {
        self.engine.scroll_to_bottom_animated();
    }

    fn set_scroll_animations_enabled(&mut self, enabled: bool) {
        self.engine.set_scroll_animations_enabled(enabled);
    }

    fn set_link_detection(&mut self, enabled: bool) {
        self.engine.set_link_detection(enabled);
    }

    fn set_image_capture(&mut self, enabled: bool) {
        self.engine.set_image_capture(enabled);
    }

    fn reload_terminal_config(
        &mut self,
        palette_defaults: Option<TerminalPaletteDefaults>,
        scrollback_size: usize,
    ) {
        self.engine.reload_config(palette_defaults, scrollback_size);
        self.force_full_damage = true;
    }
    fn lines_in_stable_range(
        &self,
        start: isize,
        end: isize,
        cols: usize,
    ) -> Vec<TerminalLine> {
        self.engine.lines_in_stable_range(start, end, cols)
    }

    fn search_scrollback(&self, query: &str, cols: usize) -> Vec<crate::backend::SearchMatch> {
        self.engine.search_scrollback(query, cols)
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

fn coalesce_terminal_damage_rows(rows: Vec<usize>) -> TerminalDamage {
    let mut ranges = Vec::new();
    let mut iter = rows.into_iter();
    let Some(mut start) = iter.next() else {
        return TerminalDamage::None;
    };
    let mut end = start + 1;

    for row in iter {
        if row == end {
            end += 1;
        } else {
            ranges.push(TerminalRowRange::new(start, end));
            start = row;
            end = row + 1;
        }
    }
    ranges.push(TerminalRowRange::new(start, end));
    TerminalDamage::Rows(ranges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::thread;
    use std::time::{Duration, Instant};

    // Generous on purpose: tests spawn real shells/TUIs (bash, nvim) whose
    // first paint can lag under parallel-test CPU contention. This is only a SAFETY
    // CAP, not a measured wait: `pump_backend_until` returns the instant its
    // predicate is true (these tests usually finish well under a second), so a
    // generous cap costs nothing on the happy path and just removes false timeouts
    // under load. 3s flaked ~1 in 3 runs; 6s still flaked when tests ran alongside
    // clippy (a real shell starved of CPU echoes after the deadline). 30s is ample.
    const TEST_TIMEOUT: Duration = Duration::from_secs(30);
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
    fn terminal_backend_initial_damage_is_full_then_none() {
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
        assert!(
            backend.reaped,
            "backend should reap the PTY child after exit"
        );
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

        assert!(
            exited,
            "failed wait should conservatively close the backend"
        );
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
        let clean_home = std::env::temp_dir()
            .join(format!("heca-bash-test-home-{}", std::process::id()));
        std::fs::create_dir_all(&clean_home).expect("create clean HOME for bash test");
        let env = vec![
            ("HOME".to_string(), clean_home.to_string_lossy().into_owned()),
            ("PATH".to_string(), std::env::var("PATH").unwrap_or_default()),
        ];
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
        let zsh = PathBuf::from("/bin/zsh");
        if !zsh.exists() || !command_exists("nvim") {
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
        let mut backend = TerminalBackend::with_command(
            80,
            24,
            8.4,
            14.0,
            "printf 'phase6-ok\\n'; exit 7",
            TerminalBackendOptions::default(),
        )
        .expect("command backend should initialize");

        assert!(
            pump_backend_until(&mut backend, |backend| backend.runtime().exit_code
                == Some(7)),
            "command backend should capture the exit code"
        );
        assert_eq!(backend.runtime().status, ProcessStatus::Error);
        assert!(
            terminal_text(&backend).contains("phase6-ok"),
            "command backend should render command output"
        );
        assert!(
            !backend.should_close(),
            "direct command backends stay open until pane close-policy decides"
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
            .map(|line| {
                line.cells
                    .iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<String>()
            })
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
            include_str!("../../../heca/assets/shell-integration/bash_init.sh"),
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
