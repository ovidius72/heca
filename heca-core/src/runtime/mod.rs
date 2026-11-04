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
/// `pane.exited{code}` event + `PaneRuntime::exit_code` (for the close-policy on
/// direct-spawn panes), not by a status.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ProcessStatus {
    #[default]
    Idle,
    Running,
    Success,
    Error,
}

impl ProcessStatus {
    /// Stable status identifier shared with configured presentation tables.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Success => "success",
            Self::Error => "error",
        }
    }

    /// **What an exit code says about a program**: `0` is success, anything else an error.
    ///
    /// The one reading of an exit code — the shell's report of a finished command, the pane's own
    /// process ending, and the close policy all ask here rather than each comparing with `0`.
    pub fn of_exit(code: i32) -> Self {
        if code == 0 {
            Self::Success
        } else {
            Self::Error
        }
    }
}

/// Source/content type hosted by a pane.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ContentKind {
    #[default]
    Terminal,
}

/// Exit-time close policy for a pane that directly spawned a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct PaneClosePolicy {
    /// Close the pane automatically when the direct command exits.
    pub close_pane: bool,
    /// Keep the pane open when the command exits with a non-zero code.
    pub keep_on_error: bool,
    /// Keep the pane open when the command exits successfully (`0`).
    pub keep_on_success: bool,
}

impl PaneClosePolicy {
    /// Decide whether a pane should close for the given exit code.
    pub fn should_close(self, code: Option<i32>) -> bool {
        if !self.close_pane {
            return false;
        }
        match code.map(ProcessStatus::of_exit) {
            Some(ProcessStatus::Error) if self.keep_on_error => return false,
            Some(ProcessStatus::Success) if self.keep_on_success => return false,
            _ => {}
        }
        true
    }
}

/// Git summary for the pane's current working directory.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct PaneRuntime {
    pub program: Option<String>,
    pub status: ProcessStatus,
    pub cwd: Option<PathBuf>,
    pub exit_code: Option<i32>,
    pub git: Option<GitInfo>,
    pub kind: ContentKind,
    /// Last report from a recognized agent, separate from the process lifecycle.
    pub agent: Option<crate::agent::AgentState>,
}

#[cfg(test)]
mod tests;
