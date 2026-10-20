//! **What the workspaces component can do** — the actions it declares on its own cursor.

use super::*;

// The ids this component declares. Written once so the declaration and `perform` cannot drift, and
// namespaced by `kind()` so two components can both have a `cursor_up`.
pub(super) const CURSOR_UP: &str = "workspaces.cursor_up";
pub(super) const CURSOR_DOWN: &str = "workspaces.cursor_down";
pub(super) const COLLAPSE_ROW: &str = "workspaces.collapse_row";
pub(super) const ACTIVATE_SELECTED: &str = "workspaces.activate_selected";
pub(super) const PEEK_SELECTED: &str = "workspaces.peek_selected";
pub(super) const TOGGLE_SELECTED: &str = "workspaces.toggle_selected";
// The mutations. Each resolves the cursor to a target and dispatches an existing action with
// arguments, so none of them reimplements anything.
pub(super) const CREATE_COLUMN: &str = "workspaces.create_column";
pub(super) const CREATE_PANE: &str = "workspaces.create_pane";
pub(super) const DELETE_SELECTED: &str = "workspaces.delete_selected";
// Renames the row the cursor is on. The app's own `rename_pane` / `rename_workspace` mean the
// focused pane and the active workspace, which is a different question.
pub(super) const RENAME_SELECTED: &str = "workspaces.rename_selected";
pub(super) const DELETE_COLUMN: &str = "workspaces.delete_selected_column";
pub(super) const ZOOM_SELECTED: &str = "workspaces.zoom_selected";

/// Every action this component declares. See [`Provider::actions`].
///
/// **The policy is per action, not per component**, because moving the cursor and deleting a
/// workspace are not the same kind of act:
/// - the cursor is chrome state, so its moves stay reachable while a floating pane is active
///   (`Global`);
/// - a verb that ends by focusing a pane cannot be `Global`, however harmless its first half looks,
///   or the picker would offer a letter that half works while a floating pane is active
///   (`SourceDependent`);
/// - everything that acts on the row under the cursor only makes sense while this dock has the
///   keyboard (`ContainerFocused`).
pub(super) fn declared() -> Vec<ActionMeta> {
    let cursor = ActionPolicy::Global;
    let reaches_a_pane = ActionPolicy::SourceDependent;
    let mutates = ActionPolicy::ContainerFocused;
    let act = |name: &str, label: &str, description: &str, icon: Glyph, policy| {
        ActionMeta::new(name)
            .label(label)
            .description(description)
            .icon(icon)
            .category(ActionCategory::Navigation)
            .policy(policy)
    };
    vec![
        act(
            CURSOR_UP,
            "Cursor Up",
            "Move the workspaces cursor to the previous row.",
            Glyph::CaretUp,
            cursor,
        ),
        act(
            CURSOR_DOWN,
            "Cursor Down",
            "Move the workspaces cursor to the next row.",
            Glyph::CaretDown,
            cursor,
        ),
        act(
            COLLAPSE_ROW,
            "Collapse Row",
            "Collapse the row under the cursor, or move out to its parent.",
            Glyph::CaretLeft,
            cursor,
        ),
        act(
            ACTIVATE_SELECTED,
            "Activate Row",
            "Expand a structural row, or focus the pane under the cursor and leave the dock.",
            Glyph::CaretRight,
            reaches_a_pane,
        ),
        // **A hint can be aimed.** It acts on the cursor by default; the picker passes the key of
        // the row whose letter was chosen, so `prefix+/` lands on that row. Optional, because the
        // keybinding, the palette and RPC mean "the row I am on".
        act(
            PEEK_SELECTED,
            "Hint Row",
            "Focus what the cursor points at without leaving the dock.",
            Glyph::Search,
            reaches_a_pane,
        )
        .arg(ArgSpec {
            name: "key".to_string(),
            kind: ArgKind::Text,
            required: false,
            description: "The row to peek, by its nav key (default: the cursor).".to_string(),
            values: Vec::new(),
        }),
        act(
            TOGGLE_SELECTED,
            "Expand / Collapse Row",
            "Fold or unfold the structural row under the cursor.",
            Glyph::List,
            cursor,
        ),
        act(
            CREATE_COLUMN,
            "Add Column",
            "Add a column to the workspace the cursor is in.",
            Glyph::ColumnsPlusRight,
            mutates,
        ),
        act(
            CREATE_PANE,
            "Add Pane",
            "Add a pane to the column the cursor is in.",
            Glyph::FolderSimplePlus,
            mutates,
        ),
        act(
            RENAME_SELECTED,
            "Rename Row",
            "Rename what the cursor is on — a pane or a workspace.",
            Glyph::NotePencil,
            mutates,
        ),
        act(
            DELETE_SELECTED,
            "Delete Row",
            "Delete what the cursor is on — a pane or a workspace.",
            Glyph::Trash,
            mutates,
        ),
        act(
            DELETE_COLUMN,
            "Delete Column",
            "Delete the column the cursor is in, and every pane in it.",
            Glyph::Trash,
            mutates,
        ),
        act(
            ZOOM_SELECTED,
            "Zoom Column",
            "Toggle zoom on the column the cursor is in.",
            Glyph::FrameCorners,
            mutates,
        ),
    ]
}
