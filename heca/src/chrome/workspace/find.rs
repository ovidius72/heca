//! **Finding things in the workspace**, for the callers that address a pane by who it is.

use heca_core::layout::PaneId;
use heca_grid_ui::Component;

use super::WORKSPACE_KEY;
use crate::app_state::AppState;

/// The workspace node in the window tree, if it has been seated.
pub(crate) fn workspace_node(state: &AppState) -> Option<&dyn Component> {
    state
        .window_root
        .base()
        .children
        .iter()
        .find(|c| c.base().key.as_deref() == Some(WORKSPACE_KEY))
        .map(|c| c.as_ref())
}

/// [`workspace_node`], to write to.
pub(crate) fn workspace_node_mut(state: &mut AppState) -> Option<&mut (dyn Component + 'static)> {
    state
        .window_root
        .base_mut()
        .children
        .iter_mut()
        .find(|c| c.base().key.as_deref() == Some(WORKSPACE_KEY))
        .map(|c| c.as_mut())
}

/// The node of pane `pane`, wherever it is held — in a column or floating.
pub(crate) fn pane_node(state: &AppState, pane: PaneId) -> Option<&dyn Component> {
    heca_grid_ui::node_with_key(workspace_node(state)?, &crate::chrome::pane_key(pane))
}

/// [`pane_node`], to write to.
pub(crate) fn pane_node_mut(state: &mut AppState, pane: PaneId) -> Option<&mut dyn Component> {
    heca_grid_ui::node_with_key_mut(workspace_node_mut(state)?, &crate::chrome::pane_key(pane))
}

/// **Offer a letter to the columns that answer to `key`** — the column itself, and the offer of a
/// new column beside it, never what is around them.
///
/// **Only where it can be seen.** `seen` answers whether a pane's view is visible — the same
/// question every other view's letter is asked. A column behind a sidebar used to be lettered
/// regardless, so a pane under the sidebar drew its keycap on top of the sidebar: the pane path
/// withdrew the letter and this path put it straight back.
pub(crate) fn offer_to_columns(
    state: &AppState,
    key: &str,
    label: Option<String>,
    seen: impl Fn(PaneId) -> bool,
) -> bool {
    let Some(workspace) = workspace_node(state) else {
        return false;
    };
    let mut offered = false;
    for column in workspace.base().children.iter().filter(|c| {
        c.base()
            .key
            .as_deref()
            .is_some_and(|k| k.starts_with("col:"))
    }) {
        let panes: Vec<PaneId> = column
            .base()
            .children
            .iter()
            .filter_map(|c| crate::chrome::pane_id_of_key(c.base().key.as_deref()?))
            .collect();
        let label = if column_view_seen(&panes, key, &seen) {
            label.clone()
        } else {
            // A withdrawal, never "leave whatever is there": a letter given while the column was
            // visible must go when it is covered.
            None
        };
        // **By key, into the tree** — the column names itself, and the offer of a new column beside
        // it is a child. Matching only the root meant a target had to BE the tree it lived in.
        offered |= heca_grid_ui::offer_hint_by_key(column.as_ref(), key, label);
    }
    offered
}

/// **Can this column show the offer for `key`?** A pane it holds is judged by that pane's own view;
/// anything else in the column — the column itself, the new-column slot beside it — by whether any
/// of its panes can be seen.
///
/// Pure, so the rule is tested without a window.
pub(crate) fn column_view_seen(panes: &[PaneId], key: &str, seen: impl Fn(PaneId) -> bool) -> bool {
    match panes
        .iter()
        .find(|p| crate::providers::workspaces::pane_key(**p) == key)
    {
        Some(pane) => seen(*pane),
        None => panes.iter().any(|p| seen(*p)),
    }
}
