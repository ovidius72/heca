//! **Starting a terminal**: a shell (with or without shell integration) or a command, the folder it
//! starts in, and the options a caller can set.

use super::engine::TerminalEngine;
use super::osc::OscSnooper;
use super::pty::PtyHandle;
use super::{PtyError, TerminalBackend, process};
use crate::backend::TerminalPaletteDefaults;
use crate::runtime::{ContentKind, PaneRuntime};
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

/// What a test shell needs beyond the [options](TerminalBackendOptions): which shell, and an
/// environment of its own. The app always starts the user's shell in the user's environment.
pub(super) struct ShellLaunch<'a> {
    pub(super) override_path: Option<&'a str>,
    /// Clear the inherited environment before spawning, isolating the shell from
    /// the user's shell config (e.g. a `~/.bashrc` that loads bash-preexec and
    /// clobbers our OSC 133 `DEBUG` trap / `PROMPT_COMMAND`). `env` is applied
    /// after the clear. Default `false` — the real app keeps the user's env.
    pub(super) env_clear: bool,
    /// Extra env vars to set on the spawned shell (after an optional
    /// `env_clear`). Used by integration tests to give the shell a clean `HOME`
    /// so the test is deterministic across machines.
    pub(super) env: Vec<(String, String)>,
}

impl ShellLaunch<'_> {
    /// The user's own shell, in the user's environment.
    const USER: Self = Self {
        override_path: None,
        env_clear: false,
        env: Vec::new(),
    };
}

/// **What runs in the terminal**: a shell, or one command. Everything else about the start — the
/// folder, shell integration, the shell a command runs under — is in the
/// [options](TerminalBackendOptions).
pub(super) enum LaunchTarget<'a> {
    Shell(ShellLaunch<'a>),
    Command(&'a str),
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
    /// Run a spawned command under **this** shell instead of the user's `$SHELL`.
    ///
    /// A command is spawned as `<shell> -ic <command>` so terminal-first programs keep the
    /// interactive shell's job control and rc — see `command_for_spawned_command`. That is right
    /// for a pane the user opened and wrong for anything that must behave the same everywhere: an
    /// interactive rc is the user's, and it may print, prompt, or block. heca's own tests set this
    /// to `/bin/sh` for exactly that reason (an `oh-my-zsh` "Would you like to update? [Y/n]"
    /// prompt made a spawn test hang for its full 30s timeout, on one machine and not another).
    ///
    /// `None` — the default — uses `$SHELL`, falling back to `/bin/sh`.
    pub shell_override: Option<String>,
    /// The folder to start in — a shell, or a spawned command. `None` — the default — is the
    /// process's own working directory. A folder that does not exist makes the spawn fail, so a
    /// caller that wants a fallback checks first.
    pub cwd: Option<std::path::PathBuf>,
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
            shell_override: None,
            cwd: None,
        }
    }
}

impl TerminalBackend {
    /// **Start a shell**, `cols` × `rows` cells of `cell_w` × `cell_h` logical px, as `options`
    /// say.
    pub fn with_options(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        options: TerminalBackendOptions,
    ) -> Result<Self, PtyError> {
        let shell = LaunchTarget::Shell(ShellLaunch::USER);
        Self::with_launch_target(cols, rows, (cell_w, cell_h), options, shell)
    }

    /// **Run `command`** in a terminal of `cols` × `rows` cells, as `options` say.
    pub fn with_command(
        cols: usize,
        rows: usize,
        cell_w: f32,
        cell_h: f32,
        command: &str,
        options: TerminalBackendOptions,
    ) -> Result<Self, PtyError> {
        let command = LaunchTarget::Command(command);
        Self::with_launch_target(cols, rows, (cell_w, cell_h), options, command)
    }

    /// The one place a terminal starts: the options, taken whole, and what runs.
    pub(super) fn with_launch_target(
        cols: usize,
        rows: usize,
        cell_size: (f32, f32),
        options: TerminalBackendOptions,
        launch: LaunchTarget<'_>,
    ) -> Result<Self, PtyError> {
        let (cell_w, cell_h) = cell_size;
        let TerminalBackendOptions {
            palette_defaults,
            wake_on_output,
            shell_integration,
            scrollback_size,
            scroll_animations,
            shell_override,
            cwd,
        } = options;
        let (pty, auto_close_on_exit) = match launch {
            LaunchTarget::Shell(shell) => {
                let pty = PtyHandle::new_with_shell(
                    cols,
                    rows,
                    cell_size,
                    wake_on_output,
                    shell_integration,
                    shell.override_path,
                    shell.env_clear,
                    &shell.env,
                    cwd.as_deref(),
                )?;
                (pty, true)
            }
            LaunchTarget::Command(command) => (
                PtyHandle::new_with_command(
                    cols,
                    rows,
                    cell_size,
                    wake_on_output,
                    command,
                    shell_override.as_deref(),
                    cwd.as_deref(),
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
            pending_pty_size: None,
            last_pty_resize: Instant::now(),
        })
    }

    /// A test shell — `shell` says which, and in what environment — started as `options` say, at
    /// the [default cell size](crate::backend::DEFAULT_CELL_SIZE).
    #[cfg(test)]
    pub(super) fn with_test_shell(
        cols: usize,
        rows: usize,
        options: TerminalBackendOptions,
        shell: ShellLaunch<'_>,
    ) -> Result<Self, PtyError> {
        let cell = crate::backend::DEFAULT_CELL_SIZE;
        Self::with_launch_target(cols, rows, cell, options, LaunchTarget::Shell(shell))
    }

    /// The shell at `shell`, with every option at its default.
    #[cfg(test)]
    pub(super) fn new_for_test_with_shell(
        cols: usize,
        rows: usize,
        shell: &str,
    ) -> Result<Self, PtyError> {
        let shell = ShellLaunch {
            override_path: Some(shell),
            ..ShellLaunch::USER
        };
        Self::with_test_shell(cols, rows, TerminalBackendOptions::default(), shell)
    }
}
