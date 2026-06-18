//! Canonical per-pane runtime state.
//!
//! This module is UI-free and lives in `heca-core` so the backend/pane layer owns
//! the authoritative runtime truth. The app-side chrome store mirrors this state
//! reactively for rendering and plugin/event consumers.

use std::path::PathBuf;

/// High-level runtime status for a pane's foreground process lifecycle.
///
/// There is **no `Exit` variant**: a pane whose terminal (shell) has died is
/// closed, so it has no status left to display. The exit *code* is carried by the
/// `pane.exited{code}` event + `PaneRuntime::exit_code` (for Phase 6 close-policy
/// on direct-spawn panes), not by a status. See `pane-runtime-state-plan.md` §0.6.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProcessStatus {
    #[default]
    Idle,
    Running,
    Success,
    Error,
}

/// Source/content type hosted by a pane.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ContentKind {
    #[default]
    Terminal,
}

/// Exit-time close policy for a pane that directly spawned a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PaneClosePolicy {
    pub close_pane: bool,
    pub keep_on_error: bool,
    pub keep_on_success: bool,
}

impl PaneClosePolicy {
    /// Decide whether a pane should close for the given exit code.
    pub fn should_close(self, code: Option<i32>) -> bool {
        if !self.close_pane {
            return false;
        }
        if self.keep_on_error && code.is_some_and(|exit| exit != 0) {
            return false;
        }
        if self.keep_on_success && code == Some(0) {
            return false;
        }
        true
    }
}

/// Git summary for the pane's current working directory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub added: u32,
    pub modified: u32,
    pub deleted: u32,
    pub dirty: bool,
}

/// Canonical runtime metadata owned by a pane/backend.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaneRuntime {
    pub program: Option<String>,
    pub status: ProcessStatus,
    pub cwd: Option<PathBuf>,
    pub exit_code: Option<i32>,
    pub git: Option<GitInfo>,
    pub kind: ContentKind,
}
