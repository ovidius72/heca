//! **How the workspaces component performs its actions** — stepping, folding and activating the
//! row under its cursor.

use super::*;

pub(super) enum Step {
    Up,
    Down,
}

pub(super) enum Fold {
    Collapse,
    Toggle,
}

pub(super) enum Activate {
    /// `Enter`/`l` — focus the row and hand the keyboard back to the pane.
    AndLeave,
    /// `Space` — focus the row in the main view but keep driving the dock.
    AndStay,
}

/// What to do to the row under the cursor. Both verbs resolve the same way — the row decides
/// whether the by-id action is the pane's or the workspace's — so they share one arm.
#[derive(Clone, Copy)]
pub(super) enum RowVerb {
    Rename,
    Delete,
}

impl WorkspacesContainerProvider {
    /// Move the cursor over this component's own rows.
    pub(super) fn step_cursor(&self, cx: &mut ProviderCx<'_>, step: Step) -> Handled {
        {
            let state = cx.state();
            let mut tree = state.workspaces().tree_mut();
            match step {
                Step::Up => tree.cursor_up(),
                Step::Down => tree.cursor_down(),
            }
        }
        Handled::Yes
    }

    /// Fold or unfold the structural row under the cursor. A leaf has nothing to fold, so `Toggle`
    /// on a pane does nothing rather than guessing at a meaning for it.
    pub(super) fn fold(&self, cx: &mut ProviderCx<'_>, fold: Fold) -> Handled {
        {
            let state = cx.state();
            let ws = state.workspaces();
            let leaf = matches!(
                ws.tree().current_item(),
                Some(WorkspaceRow::Pane { .. }) | Some(WorkspaceRow::FloatingPane { .. })
            );
            match fold {
                // `h` on a leaf still means "out to my parent", which `collapse` implements.
                Fold::Collapse => ws.tree_mut().collapse(ws),
                Fold::Toggle if !leaf => ws.tree_mut().toggle_expand(ws),
                Fold::Toggle => {}
            }
        }
        Handled::Yes
    }

    /// Focus what the cursor points at. A structural row expands instead — `l` on a collapsed
    /// workspace opens it, which is what makes one key walk the tree outward.
    pub(super) fn activate(&self, cx: &mut ProviderCx<'_>, mode: Activate) -> Handled {
        // Read, then drop the borrow: `dispatch` reaches back into the store.
        let row = {
            let state = cx.state();
            state.workspaces().tree().current_item().cloned()
        };
        let Some(row) = row else {
            return Handled::No;
        };
        match row {
            WorkspaceRow::Pane { pane_id } | WorkspaceRow::FloatingPane { pane_id, .. } => {
                cx.dispatch(
                    "focus_pane",
                    PropMap::from([("pane_id".to_string(), PropValue::Int(pane_id.0 as i64))]),
                );
                // Activating a leaf hands the keyboard back; hinting keeps it here.
                if matches!(mode, Activate::AndLeave) {
                    cx.dispatch("unfocus_dock", PropMap::new());
                }
            }
            WorkspaceRow::Workspace { ws_idx, .. } => {
                let collapsed = {
                    let state = cx.state();
                    let ws = state.workspaces();
                    let collapsed = ws.is_ws_collapsed(ws_idx);
                    // Expanding is this component's own business — no app state involved.
                    if collapsed && matches!(mode, Activate::AndLeave) {
                        ws.tree_mut().expand(ws);
                    }
                    collapsed
                };
                if !(collapsed && matches!(mode, Activate::AndLeave)) {
                    cx.dispatch(
                        "focus_workspace",
                        PropMap::from([("ws_idx".to_string(), PropValue::Int(ws_idx as i64))]),
                    );
                }
            }
            // The cursor never lands on a column (`WorkspaceTree::is_navigable`).
            WorkspaceRow::Column { .. } => return Handled::No,
        }
        Handled::Yes
    }

    // ── The mutations ──
    //
    // **"New workspace" is not here on purpose.** It is about no row, so it is not this component's
    // to declare — `create_workspace` already exists and the component simply BINDS it (`w`), which
    // is the same rule that keeps `zoom_column`, `close` and `next_pane` out of the list above.
    // Declaring `workspaces.create_workspace` would also have been unreachable: a short name that
    // matches a built-in resolves to the built-in, by design.
    //
    // Every one of these does the same two things: resolve **this component's cursor** to a target,
    // then dispatch an action that already exists with that target as arguments. The component
    // contributes the only part nothing else can know — which row the cursor is on — and reuses the
    // rest. That is why `zoom_column_at_index` had to be added: without a parameterized form there
    // was nothing to dispatch, and "zoom the selected column" could only be a handler reaching into
    // this component's model from outside.

    /// The workspace the cursor is in, whatever kind of row it is on.
    pub(super) fn cursor_workspace(&self, cx: &ProviderCx<'_>) -> Option<usize> {
        let state = cx.state();
        let tree = state.workspaces().tree();
        match tree.current_item()? {
            WorkspaceRow::Workspace { ws_idx, .. }
            | WorkspaceRow::Column { ws_idx, .. }
            | WorkspaceRow::FloatingPane { ws_idx, .. } => Some(*ws_idx),
            WorkspaceRow::Pane { pane_id } => tree.locate_pane(*pane_id).map(|(ws, _)| ws),
        }
    }

    /// The column the cursor is in — `None` on a workspace row or a floating pane, neither of which
    /// is inside a column.
    pub(super) fn cursor_column(&self, cx: &ProviderCx<'_>) -> Option<(usize, usize)> {
        let state = cx.state();
        let tree = state.workspaces().tree();
        match tree.current_item()? {
            WorkspaceRow::Column {
                ws_idx, col_idx, ..
            } => Some((*ws_idx, *col_idx)),
            WorkspaceRow::Pane { pane_id } => tree.locate_pane(*pane_id),
            WorkspaceRow::Workspace { .. } | WorkspaceRow::FloatingPane { .. } => None,
        }
    }

    /// Dispatch `action` with the cursor's workspace.
    pub(super) fn at_workspace(&self, cx: &mut ProviderCx<'_>, action: &str) -> Handled {
        // Read, then drop the borrow: `dispatch` reaches back into the store.
        let Some(ws_idx) = self.cursor_workspace(cx) else {
            return Handled::No;
        };
        cx.dispatch(
            action,
            PropMap::from([("ws_idx".to_string(), PropValue::Int(ws_idx as i64))]),
        );
        Handled::Yes
    }

    /// Dispatch `action` with the cursor's column. A cursor that is not in one declines — a silent
    /// decline is a real answer, not an error.
    pub(super) fn at_column(&self, cx: &mut ProviderCx<'_>, action: &str) -> Handled {
        let Some((ws_idx, col_idx)) = self.cursor_column(cx) else {
            return Handled::No;
        };
        cx.dispatch(
            action,
            PropMap::from([
                ("ws_idx".to_string(), PropValue::Int(ws_idx as i64)),
                ("col_idx".to_string(), PropValue::Int(col_idx as i64)),
            ]),
        );
        Handled::Yes
    }

    /// Act on what the cursor is **on** — a pane or a workspace — with the by-id action for
    /// whichever it is.
    ///
    /// Never a column: the cursor does not land on one (`WorkspaceTree::is_navigable`), which is why
    /// `workspaces.delete_selected_column` is a separate verb rather than a case here.
    ///
    /// Every one goes out as an ordinary dispatch, so the central destructive gate raises the same
    /// confirm it does for a keybinding, a header button or an RPC call — the component neither asks
    /// nor can skip it.
    pub(super) fn on_selected_row(&self, cx: &mut ProviderCx<'_>, verb: RowVerb) -> Handled {
        let row = {
            let state = cx.state();
            state.workspaces().tree().current_item().cloned()
        };
        // `(action, argument name, argument value)` — the row decides which of the two by-id
        // actions the verb means, and what to aim it at.
        let (action, arg, value) = match (verb, row) {
            (RowVerb::Delete, Some(WorkspaceRow::Pane { pane_id }))
            | (RowVerb::Delete, Some(WorkspaceRow::FloatingPane { pane_id, .. })) => {
                ("close_pane_by_id", "pane_id", pane_id.0 as i64)
            }
            (RowVerb::Rename, Some(WorkspaceRow::Pane { pane_id }))
            | (RowVerb::Rename, Some(WorkspaceRow::FloatingPane { pane_id, .. })) => {
                ("rename_pane_by_id", "pane_id", pane_id.0 as i64)
            }
            (RowVerb::Delete, Some(WorkspaceRow::Workspace { ws_idx, .. })) => {
                ("delete_workspace", "ws_idx", ws_idx as i64)
            }
            (RowVerb::Rename, Some(WorkspaceRow::Workspace { ws_idx, .. })) => {
                ("rename_workspace_by_idx", "ws_idx", ws_idx as i64)
            }
            (_, Some(WorkspaceRow::Column { .. })) | (_, None) => return Handled::No,
        };
        cx.dispatch(
            action,
            PropMap::from([(arg.to_string(), PropValue::Int(value))]),
        );
        Handled::Yes
    }

    /// Point the list at the row this placement selected — the one record of where its cursor is.
    pub(super) fn load_cursor(&self, cx: &ProviderCx<'_>) {
        let key = cx.selected();
        cx.state().workspaces().tree_mut().point_at(key.as_deref());
    }

    /// Write the row the list now stands on back as this placement's selection.
    pub(super) fn publish_cursor(&self, cx: &mut ProviderCx<'_>) {
        let selection = cx.state().workspaces().tree().selection();
        cx.set_selected(selection.map(selection_key));
    }
}
