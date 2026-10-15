//! **The workspaces component's own right-click menus.**

use super::*;

// ── This component's context menus ──
//
// **A container's menu is the component's, not the host's.** These three used to be registered in
// `ContextMenuRegistry::with_builtins` under `sidebar.pane` / `sidebar.column` / `sidebar.workspace`
// — paths named for *where the rows are drawn*, which is the same defect `SidebarTree` and the
// `sidebar_*` actions had. They are the component's own paths now, namespaced like its action ids,
// and a Docker dock names its rows `docker.container` with no host constant involved.

/// A pane row of this component.
pub(crate) const MENU_PANE: &str = "workspaces.pane";
/// A column row of this component.
pub(crate) const MENU_COLUMN: &str = "workspaces.column";
/// A workspace row of this component.
pub(crate) const MENU_WORKSPACE: &str = "workspaces.workspace";
/// The container itself — the menu for empty space, found by bubbling when no row claims the click.
pub(crate) const MENU_CONTAINER: &str = "workspaces.container";

/// Menu items for the container's **empty space**: the verbs that are about the list rather than
/// about any row in it. Facts-only, so it is unit-testable without a ctx, like its row siblings.
pub(crate) fn container_items() -> Vec<DropdownItem> {
    vec![item("create_workspace", "New workspace")]
}

/// The row this component's `key` names, or `None` for a key it did not write.
///
/// **Matched, never parsed** — the one resolution every reader of a key goes through here (the
/// cursor, the menu path, the menu itself), so they cannot disagree about what a key points at.
pub(super) fn row_at_key(tree: &WorkspaceTree, key: &str) -> Option<WorkspaceRow> {
    tree.flat_items
        .iter()
        .find(|row| selection_key(row.selection()) == key)
        .cloned()
}

/// Menu items for a **pane** row (facts-only, so unit-testable without a ctx). Every entry acts on
/// *that* row: the pane by id, "New pane" in the pane's own column when it has one.
pub(crate) fn pane_row_items(
    pane_id: PaneId,
    column: Option<(usize, usize)>,
    has_custom_name: bool,
) -> Vec<DropdownItem> {
    let pane = PropValue::Int(pane_id.0 as i64);
    let mut items = Vec::new();
    if let Some((ws_idx, col_idx)) = column {
        items.push(item_running(
            "add_pane_to_column",
            "New pane",
            "add_pane_to_column",
            &[
                ("ws_idx", usize_arg(ws_idx)),
                ("col_idx", usize_arg(col_idx)),
            ],
        ));
    }
    items.push(item_running(
        "rename_pane",
        "Rename pane",
        "rename_pane_by_id",
        &[("pane_id", pane.clone())],
    ));
    if has_custom_name {
        items.push(item_running(
            "reset_pane_name",
            "Use process name",
            "reset_pane_name_by_id",
            &[("pane_id", pane.clone())],
        ));
    }
    items.push(
        item_running(
            "close",
            "Close pane",
            "close_pane_by_id",
            &[("pane_id", pane)],
        )
        .danger(true),
    );
    items
}

/// Menu items for a **column** row (facts-only, so unit-testable without a ctx).
pub(crate) fn column_row_items(ws_idx: usize, col_idx: usize) -> Vec<DropdownItem> {
    vec![
        item_running(
            "add_pane_to_column",
            "New pane",
            "add_pane_to_column",
            &[
                ("ws_idx", usize_arg(ws_idx)),
                ("col_idx", usize_arg(col_idx)),
            ],
        ),
        item_running(
            "split_horizontal",
            "New column",
            "add_column_to_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
        // NB: no "Rename column" entry — a column's name is not displayed anywhere yet (columns
        // render as a MarkerGroup with no header/label), so renaming would have no visible effect.
        // The rename action stays wired (RPC + handler) for when columns surface a name.
        item_running(
            "delete_column",
            "Delete column",
            "delete_column",
            &[
                ("ws_idx", usize_arg(ws_idx)),
                ("col_idx", usize_arg(col_idx)),
            ],
        )
        .danger(true),
    ]
}

/// Menu items for a **workspace** row. `has_custom_name` gates the "Use default name"
/// reset entry — it only appears when there is a custom name to clear.
pub(crate) fn workspace_row_items(ws_idx: usize, has_custom_name: bool) -> Vec<DropdownItem> {
    let mut items = vec![
        item_running(
            "split_horizontal",
            "New column",
            "add_column_to_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
        item("create_workspace", "New workspace"),
        item_running(
            "rename_workspace",
            "Rename workspace",
            "rename_workspace_by_idx",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
    ];
    if has_custom_name {
        items.push(item_running(
            "reset_workspace_name",
            "Use default name",
            "reset_workspace_name_by_idx",
            &[("ws_idx", usize_arg(ws_idx))],
        ));
    }
    items.push(
        item_running(
            "delete_workspace",
            "Delete workspace",
            "delete_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        )
        .danger(true),
    );
    items
}
