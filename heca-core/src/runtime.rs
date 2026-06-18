//! Canonical per-pane runtime state.
//!
//! This module is UI-free and lives in `heca-core` so the backend/pane layer owns
//! the authoritative runtime truth. The app-side chrome store mirrors this state
//! reactively for rendering and plugin/event consumers.

use std::path::PathBuf;

/// High-level runtime status for a pane's foreground process lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProcessStatus {
    #[default]
    Idle,
    Running,
    Success,
    Error,
    Exit,
}

/// Source/content type hosted by a pane.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ContentKind {
    #[default]
    Terminal,
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
