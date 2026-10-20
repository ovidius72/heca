//! Shared chrome/UI state, split along the **shell vs container** boundary
//! (`read-via-signals / write-via-actions`).
//!
//! The sidebar is a content-agnostic **shell** hosting movable **containers**, so the state
//! splits the same way:
//!
//! - [`SharedChromeState`] (`shell.rs`) = **container-agnostic shell state**: each region's
//!   visibility and size, and each mount's own scroll offset, cursor and keyboard focus.
//!   NOTHING workspace/pane-specific lives here.
//! - [`WorkspacesContainerState`] (`workspaces.rs`) = the **workspaces container's own** state:
//!   its model, per-workspace collapse, the active pane, the pick in progress and the settings it
//!   renders with. Held by `SharedChromeState.workspaces`; it travels with the container when it
//!   moves between regions.
//! - Each pane's live facts (`pane_runtime.rs`): program, status, folder, git and terminal
//!   viewport, one signal per field.
//!
//! Canonical *session* state stays in `Session`; this is derived UI state + ids.
//!
//! ## Access contract
//! - **Write** only through `set_*` / `toggle_*` methods. Signal fields are
//!   `pub(crate)` (the chrome-build code binds them to widgets), never `pub`.
//! - **Read** via the `&self` accessors; collection signals use `.with(...)`
//!   (borrow), never `.get()` (which clones the whole collection).
//!
//! ## Threading / Clone
//! `Rc`/thread-local-runtime backed → **`!Send + !Sync`** (UI thread only). `Clone`
//! is a **shallow alias** (same backing signals), which is why these types are not
//! `Copy`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::app_state::PendingPick;
use heca_core::layout::PaneId;
use heca_core::runtime::{ContentKind, GitInfo, PaneRuntime, ProcessStatus};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, SignalWith, signal};

use super::{ChromeEvent, ChromeEventBus, RegionId, RegionMap};

pub(super) mod events;
mod pane_runtime;
mod shell;
mod workspaces;

pub use pane_runtime::PaneRuntimeStore;
pub use shell::SharedChromeState;
pub use workspaces::WorkspacesContainerState;

#[cfg(test)]
mod tests;
