//! Shared pane-operation helpers.
//!
//! Removing a pane from a workspace by id.

use heca_core::layout::{Pane, PaneId, WorkspaceMut};

/// Information about a pane removed from a workspace.
#[derive(Debug)]
pub(crate) struct RemovedPane {
    pub pane: Pane,
}

/// Remove a pane from a workspace by pane id.
///
/// Returns basic removal info (just the pane).
pub(crate) fn remove_pane_by_id(ws: &mut WorkspaceMut<'_>, pane_id: PaneId) -> Option<RemovedPane> {
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        if let Some(pi) = col.panes.iter().position(|p| p.id == pane_id) {
            let pane = ws.scroll_mut().remove_pane(ci, pi)?;
            return Some(RemovedPane { pane });
        }
    }
    None
}
