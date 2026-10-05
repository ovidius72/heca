//! Renaming a pane, column or workspace, resetting a custom name, and the rename dialog.

use crate::app::interaction::focused_pane_id;
use crate::app_state::{AppState, RenameTarget};
use crate::input::WmAction;
use heca_core::layout::PaneId;

/// Open the rename dialog for `target`, pre-filled with `current_name`. A host-owned modal
/// (`OverlayHost::open_modal`) with a single `Input` field (bound to the `"name"` form field)
/// and **OK / Cancel** buttons; the `Dialog` owns focus/keyboard (Tab / Shift+Tab between the
/// field and buttons, Enter submits, Esc cancels). On OK the new name is set by the
/// action that names it, the same one a key, a plugin or RPC runs; Cancel / Esc leaves the item unchanged. Replaces the old bottom-bar
/// `InputMode::Rename` flow.
fn open_rename_dialog(state: &mut AppState, target: RenameTarget, current_name: String) {
    use crate::chrome::PropValue;
    let action = target.action_name();
    let title = state
        .action_catalog
        .label(action)
        .unwrap_or(action)
        .to_string();
    let body = heca_view::build::Input::new()
        .value(current_name)
        .name("name")
        .into();
    let spec = crate::chrome::ModalSpec {
        title,
        body,
        actions: vec![
            // OK is disabled while the name field is blank — submission cannot be blank.
            crate::chrome::ModalAction::new("ok", "OK").disabled_when_empty("name"),
            crate::chrome::ModalAction::new("cancel", "Cancel"),
        ],
        danger: false,
        dismissible: true,
    };
    crate::chrome::open_modal(state, spec, move |state, registry, result| {
        if let crate::chrome::ModalResult::Action { id, data } = result
            && id == "ok"
        {
            let name = data
                .get("name")
                .and_then(PropValue::as_text)
                .unwrap_or_default()
                .trim()
                .to_string();
            // Submission cannot be blank — a blank OK leaves the item's name unchanged.
            if !name.is_empty() {
                registry.execute(&target.rename_to(name), state);
            }
        }
    });
}

/// Open the rename dialog for a pane, pre-filled with its current custom name (empty when it
/// is still tracking the process name). Shared by the focused-pane, by-id, and sidebar-targeted
/// rename entry points.
fn enter_pane_rename(state: &mut AppState, pane_id: PaneId) {
    // Prefill with the pane's **custom** name only — a pane that merely tracks its process name
    // renames from *blank* (the process name is not the pane's name; it's shown as a separate
    // label). Editing an existing custom name keeps it.
    let current_name = state
        .session
        .workspaces
        .iter()
        .find_map(|ws| ws.find_pane(pane_id))
        .and_then(|p| p.custom_name.clone())
        .unwrap_or_default();
    open_rename_dialog(state, RenameTarget::Pane(pane_id), current_name);
}

/// Rename the **focused** pane — and only ever that (F003/P086/T365, user decision 2026-07-30).
///
/// It used to mean the cursor's row while a dock held the keyboard, by reading a pane id off a
/// context target the host had stashed — the host resolving a fact about a row only the component
/// that drew it can see. Renaming the row under the cursor is that component's own verb now
/// (`workspaces.rename_selected`, bound to `r`), so this key keeps one meaning wherever the
/// keyboard is.
pub fn handle_rename_pane(state: &mut AppState, _action: &WmAction) {
    if let Some(pane_id) = focused_pane_id(state) {
        enter_pane_rename(state, pane_id);
    }
}

/// Enter rename mode for a specific pane by id — the context-menu / RPC entry point
/// (`RenamePaneById`), which carries its target explicitly rather than using the focused pane.
pub fn handle_rename_pane_by_id(state: &mut AppState, action: &WmAction) {
    let WmAction::RenamePaneById { pane_id } = action else {
        return;
    };
    enter_pane_rename(state, *pane_id);
}

/// Open the rename dialog for column `col_idx` in workspace `ws_idx`, pre-filled with its
/// current name. Shared by the active-column (`RenameColumn`) and by-index
/// (`RenameColumnByIdx`) entry points. Like a workspace, a column's shown label is its
/// explicit `name` if set, else the computed default `Column N` (not stored — `col.name`
/// stays `None` until renamed), so that computed label is what you edit.
fn enter_column_rename(state: &mut AppState, ws_idx: usize, col_idx: usize) {
    let current_name = state
        .session
        .workspaces
        .get(ws_idx)
        .and_then(|ws| ws.scrolling.columns.get(col_idx))
        .map(|col| {
            col.name
                .clone()
                .unwrap_or_else(|| format!("Column {}", col_idx + 1))
        })
        .unwrap_or_default();
    open_rename_dialog(
        state,
        RenameTarget::Column { ws_idx, col_idx },
        current_name,
    );
}

pub fn handle_rename_column(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.layout().active_workspace_idx();
    let Some(col_idx) = state.layout().active_workspace()
        .map(|ws| ws.scroll().active_column_idx())
    else {
        return;
    };
    enter_column_rename(state, ws_idx, col_idx);
}

/// Enter rename mode for a specific column by index — the context-menu / RPC entry point
/// (`RenameColumnByIdx`), which carries its target explicitly rather than using the active one.
pub fn handle_rename_column_by_idx(state: &mut AppState, action: &WmAction) {
    let WmAction::RenameColumnByIdx { ws_idx, col_idx } = action else {
        return;
    };
    enter_column_rename(state, *ws_idx, *col_idx);
}

/// Open the rename dialog for workspace `ws_idx`, pre-filled with its current name. Shared by
/// the active-workspace, by-index, and sidebar-targeted rename entry points.
fn enter_workspace_rename(state: &mut AppState, ws_idx: usize) {
    // Prefill with the workspace's **displayed** label — its explicit `name` if set, otherwise the
    // computed default `Workspace N` (there is no separate "process" concept for a workspace, so the
    // shown label is what you edit). NB: the default label is not stored in `AppState`; `ws.name`
    // stays `None` until you rename, which is why prefilling only `ws.name` would show blank.
    let current_name = state
        .session
        .workspaces
        .get(ws_idx)
        .map(|ws| {
            ws.name
                .clone()
                .unwrap_or_else(|| format!("Workspace {}", ws_idx + 1))
        })
        .unwrap_or_default();
    open_rename_dialog(state, RenameTarget::Workspace(ws_idx), current_name);
}

/// Rename the **active** workspace — the counterpart of [`handle_rename_pane`], and unbent for the
/// same reason: the cursor's row belongs to `workspaces.rename_selected` (`r`).
pub fn handle_rename_workspace(state: &mut AppState, _action: &WmAction) {
    enter_workspace_rename(state, state.layout().active_workspace_idx());
}

/// Enter rename mode for a specific workspace by index — the context-menu / RPC entry point
/// (`RenameWorkspaceByIdx`), which carries its target explicitly rather than using the active one.
pub fn handle_rename_workspace_by_idx(state: &mut AppState, action: &WmAction) {
    let WmAction::RenameWorkspaceByIdx { ws_idx } = action else {
        return;
    };
    enter_workspace_rename(state, *ws_idx);
}

#[cfg(test)]
mod tests {
    use crate::actions::ActionCatalog;
    use crate::app_state::RenameTarget;
    use heca_core::layout::PaneId;

    /// The dialog's title is the renaming action's catalogued label, so each target has to name an
    /// action that is in the catalog — or the title silently falls back to a raw id.
    #[test]
    fn every_rename_target_is_titled_by_a_catalogued_action() {
        let catalog = ActionCatalog::with_builtins();
        for target in [
            RenameTarget::Pane(PaneId(1)),
            RenameTarget::Column {
                ws_idx: 0,
                col_idx: 0,
            },
            RenameTarget::Workspace(0),
        ] {
            let name = target.action_name();
            assert!(
                catalog.label(name).is_some(),
                "{target:?} is titled by `{name}`, which is not a catalogued action"
            );
        }
    }
}
