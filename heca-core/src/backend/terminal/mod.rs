//! Terminal backend using `portable-pty` for process management and
//! `wezterm-term` for terminal emulation.
//!
//! | File | What it owns |
//! |---|---|
//! | `launch` | starting a terminal: a shell or a command, its folder, its options |
//! | `program` | following the program inside: what is in front, what the shell says, its end |
//! | `size` | telling the program its size, at a paced rate |
//! | `pane` | everything the terminal answers as a pane |
//! | `engine` | the emulator |
//! | `pty` | the pseudo-terminal the program runs on |
//! | `osc` | reading the shell's escape codes |
//! | `process` | finding the foreground process |

// Named only in the docs below.
#[cfg(doc)]
use crate::backend::PaneBackend;
mod engine;
mod launch;
mod osc;
mod pane;
mod process;
mod program;
mod pty;
mod size;
#[cfg(test)]
mod tests;

pub use launch::{ShellIntegrationAssets, TerminalBackendOptions};
pub use pty::PtyError;

use crate::runtime::PaneRuntime;
use engine::TerminalEngine;
use osc::OscSnooper;
use pty::PtyHandle;
use std::time::Instant;

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
    /// in [`update`](PaneBackend::update) (event-driven) and exposed via [`PaneBackend::runtime`].
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
    /// A grid size the **child** has not been told about yet. See
    /// [`PTY_RESIZE_MIN_INTERVAL`](size::PTY_RESIZE_MIN_INTERVAL).
    pending_pty_size: Option<(usize, usize)>,
    /// When the child was last sent a `TIOCSWINSZ`.
    last_pty_resize: Instant,
}
