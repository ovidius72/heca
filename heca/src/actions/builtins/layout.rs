//! The built-in Layout actions, in the order they are listed.

use super::ActionDescriptor;
use crate::actions::Side;
use crate::args::{ArgDescriptor, ArgKind};
use heca_grid_ui::Glyph;

pub(super) const ACTIONS: &[ActionDescriptor] = &[
    ActionDescriptor {
        name: "split_horizontal",
        side: Side::Server,
        label: "New Column",
        description: "Create a new column to the right.",
        // New column opens to the right (see the description); ColumnsPlusLeft stays
        // in the Glyph set for a future "add column to the left" action.
        icon: Some(Glyph::ColumnsPlusRight),
        args: &[],
    },
    ActionDescriptor {
        name: "split_vertical",
        side: Side::Server,
        label: "New Pane",
        description: "Add a new pane below the current one in the same column.",
        icon: Some(Glyph::SquareHalfBottom),
        args: &[],
    },
    ActionDescriptor {
        // Add-pane-to-a-specific-column (the pane-header "+" button and the sidebar
        // column "New Pane" entry, which target a column by index — distinct from
        // `split_vertical` which splits the active column). Menu/button-only, so no
        // binding; the "+" button (`split` in `chrome/pane_items/buttons.rs`) names
        // `split_vertical` for its tooltip, so it still shows the `v` hint.
        name: "add_pane_to_column",
        side: Side::Server,
        label: "Add Pane to Column",
        description: "Add a new pane to this column.",
        icon: Some(Glyph::FolderSimplePlus),
        args: &[
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the workspace holding the column.",
            ),
            ArgDescriptor::required(
                "col_idx",
                ArgKind::Int,
                "Index of the column to add the pane to.",
            ),
        ],
    },
    ActionDescriptor {
        name: "zoom_column",
        side: Side::Server,
        label: "Toggle Column Zoom",
        description: "Toggle the active column between viewport-wide zoom and its previous width.",
        icon: Some(Glyph::FrameCorners),
        args: &[],
    },
    ActionDescriptor {
        name: "open_context_menu",
        side: Side::Client,
        label: "Open Context Menu",
        description: "Open the focused pane's context menu at the cursor.",
        icon: Some(Glyph::DotsThreeVertical),
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_view_left",
        side: Side::Client,
        label: "Scroll View Left",
        description: "Pan the horizontal view left to reach off-screen / overflowing columns.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_view_right",
        side: Side::Client,
        label: "Scroll View Right",
        description: "Pan the horizontal view right to reach off-screen / overflowing columns.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "resize_increase",
        side: Side::Server,
        label: "Increase Column Width",
        description: "Widen the active column.",
        icon: Some(Glyph::Plus),
        args: &[],
    },
    ActionDescriptor {
        name: "resize_decrease",
        side: Side::Server,
        label: "Decrease Column Width",
        description: "Narrow the active column.",
        icon: Some(Glyph::Minus),
        args: &[],
    },
    ActionDescriptor {
        name: "pane_height_increase",
        side: Side::Server,
        label: "Increase Pane Height",
        description: "Tallens the active pane within its column.",
        icon: Some(Glyph::StackPlus),
        args: &[],
    },
    ActionDescriptor {
        name: "pane_height_decrease",
        side: Side::Server,
        label: "Decrease Pane Height",
        description: "Shortens the active pane within its column.",
        icon: Some(Glyph::StackMinus),
        args: &[],
    },
    // ── Font zoom ──
    ActionDescriptor {
        name: "swap_left",
        side: Side::Server,
        label: "Swap Column Left",
        description: "Swap the active column with the one to its left.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "swap_right",
        side: Side::Server,
        label: "Swap Column Right",
        description: "Swap the active column with the one to its right.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "swap_up",
        side: Side::Server,
        label: "Swap Pane Up",
        description: "Swap the active pane with the one above.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "swap_down",
        side: Side::Server,
        label: "Swap Pane Down",
        description: "Swap the active pane with the one below.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "move_pane_left",
        side: Side::Server,
        label: "Move Pane to Column Left",
        description: "Move the active pane into the column on the left.",
        icon: Some(Glyph::ArrowLineLeft),
        args: &[ArgDescriptor::optional(
            "pane_id",
            ArgKind::Int,
            "The pane to move; omit for the focused one.",
        )],
    },
    ActionDescriptor {
        name: "move_pane_right",
        side: Side::Server,
        label: "Move Pane to Column Right",
        description: "Move the active pane into the column on the right.",
        icon: Some(Glyph::ArrowLineRight),
        args: &[ArgDescriptor::optional(
            "pane_id",
            ArgKind::Int,
            "The pane to move; omit for the focused one.",
        )],
    },
    ActionDescriptor {
        name: "zoom_column_at_index",
        side: Side::Server,
        label: "Zoom Column",
        description: "Toggle zoom on the named column, making it active first.",
        // Same glyph as `zoom_column`: one act, two ways of naming its target.
        icon: Some(Glyph::FrameCorners),
        args: &[
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the workspace holding the column.",
            ),
            ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to zoom."),
        ],
    },
    ActionDescriptor {
        name: "delete_current_column",
        side: Side::Client,
        label: "Delete Current Column",
        description: "Delete the focused pane's column (or the sidebar selection's) and all its panes, after asking.",
        icon: Some(Glyph::Trash),
        args: &[],
    },
    ActionDescriptor {
        name: "delete_column",
        side: Side::Server,
        label: "Delete Column",
        description: "Delete the named column and all its panes.",
        icon: Some(Glyph::Trash),
        args: &[
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the workspace holding the column.",
            ),
            ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to delete."),
        ],
    },
    ActionDescriptor {
        name: "move_pane_to_workspace_pick",
        side: Side::Client,
        label: "Move Pane to Workspace",
        description: "Select a workspace to move the active pane to.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "move_column_to_workspace_pick",
        side: Side::Client,
        label: "Move Column to Workspace",
        description: "Select a workspace to move the active column to.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "move_pane_to_column_pick",
        side: Side::Client,
        label: "Move Pane to Column",
        description: "Select a column to move the active pane into.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "move_pane_to_new_column",
        side: Side::Server,
        label: "Move Pane to New Column",
        description: "Take the active pane out of its column into a new one beside it.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "rename_column",
        side: Side::Client,
        label: "Rename Column",
        description: "Rename the active column.",
        icon: Some(Glyph::NotePencil),
        args: &[],
    },
    // ── Selection (host capability) ──
    // Column movement — bound by default, but they had no descriptor at all until
    // action-task-E went looking for actions the catalog could not see.
    ActionDescriptor {
        name: "move_column_up",
        side: Side::Server,
        label: "Move Column to Workspace Above",
        description: "Move the focused column one position earlier.",
        icon: Some(Glyph::CaretUp),
        args: &[],
    },
    ActionDescriptor {
        name: "move_column_down",
        side: Side::Server,
        label: "Move Column to Workspace Below",
        description: "Move the focused column one position later.",
        icon: None,
        args: &[],
    },
    // ── Actions that name their target (action-task-E) ──
    //
    // These are built by name + arguments through `build_action` — from a `config.toml`
    // binding with an `args` table, a menu entry's `Intent`, a mouse handler, or RPC. They
    // never carry a bare default keybinding, because a key cannot supply a pane id.
    //
    // Until action-task-E they were the only dispatchable actions with **no descriptor at
    // all**: `list-actions` could not see them, they had no label or category, and nothing
    // anywhere said what arguments they took. The `args` below are that missing half; the
    // labels and categories come with it, since metadata lives in one place per action.
    ActionDescriptor {
        name: "swap",
        side: Side::Server,
        label: "Swap Panes",
        description: "Swap the positions of two panes.",
        icon: None,
        args: &[
            ArgDescriptor::required("a_id", ArgKind::Int, "The first pane."),
            ArgDescriptor::required(
                "b_id",
                ArgKind::Int,
                "The second pane, which takes the first one's place.",
            ),
        ],
    },
    ActionDescriptor {
        name: "move",
        side: Side::Server,
        label: "Move Pane to Column",
        description: "Move a pane into another column of the current workspace.",
        icon: None,
        args: &[
            ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to move."),
            ArgDescriptor::required(
                "target_col",
                ArgKind::Int,
                "Index of the column to move it into.",
            ),
        ],
    },
    ActionDescriptor {
        name: "move_pane_to_workspace",
        side: Side::Server,
        label: "Move Pane to Workspace",
        description: "Move a pane into another workspace.",
        icon: None,
        args: &[
            ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to move."),
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the destination workspace.",
            ),
        ],
    },
    ActionDescriptor {
        name: "move_pane_to_column",
        side: Side::Server,
        label: "Move Pane to Column in Workspace",
        description: "Move a pane into a specific column of a specific workspace.",
        icon: None,
        args: &[
            ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to move."),
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the destination workspace.",
            ),
            ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the destination column."),
        ],
    },
    ActionDescriptor {
        name: "place_pane",
        side: Side::Server,
        label: "Place Pane",
        description: "Put a pane at an exact place: a row of a column, or a new column of its own.",
        icon: None,
        args: &[
            ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to place."),
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the destination workspace.",
            ),
            ArgDescriptor::required(
                "col_idx",
                ArgKind::Int,
                "Index of the destination column (past the end = the end).",
            ),
            ArgDescriptor::optional(
                "pane_idx",
                ArgKind::Int,
                "Row inside that column; omitted = a new column of its own at col_idx.",
            ),
        ],
    },
    ActionDescriptor {
        name: "move_column",
        side: Side::Server,
        label: "Move Column",
        description: "Move a column to another position, in this workspace or another one.",
        icon: None,
        args: &[
            ArgDescriptor::required(
                "src_ws",
                ArgKind::Int,
                "Index of the workspace the column is in.",
            ),
            ArgDescriptor::required("src_col", ArgKind::Int, "Index of the column to move."),
            ArgDescriptor::required(
                "dst_ws",
                ArgKind::Int,
                "Index of the destination workspace.",
            ),
            ArgDescriptor::required("dst_idx", ArgKind::Int, "Position to insert it at."),
            ArgDescriptor::optional(
                "focus",
                ArgKind::Bool,
                "Follow the column with focus. Default true.",
            ),
        ],
    },
    ActionDescriptor {
        name: "swap_columns",
        side: Side::Server,
        label: "Swap Columns",
        description: "Swap the positions of two columns.",
        icon: None,
        args: &[
            ArgDescriptor::required("a_ws", ArgKind::Int, "Workspace of the first column."),
            ArgDescriptor::required("a_col", ArgKind::Int, "Index of the first column."),
            ArgDescriptor::required("b_ws", ArgKind::Int, "Workspace of the second column."),
            ArgDescriptor::required("b_col", ArgKind::Int, "Index of the second column."),
        ],
    },
    ActionDescriptor {
        name: "resize",
        side: Side::Server,
        label: "Resize",
        description: "Move a boundary of the focused column or pane. The target decides the axis: a column is resized across, a pane down.",
        icon: None,
        args: &[
            ArgDescriptor::required_enum(
                "target",
                <crate::input::ResizeTarget as crate::args::EnumArg>::VALUES,
                "What to resize.",
            ),
            ArgDescriptor::required(
                "amount",
                ArgKind::Float,
                "How far to move the boundary, and which way: positive is right for a column, down for a pane. A pane's boundary is the one below it, or the one above when it is last — so the divider moves the same way whichever pane is active. Thousandths of the working width for a column; logical pixels for a pane.",
            ),
            ArgDescriptor::optional_enum(
                "edge",
                <crate::input::ResizeEdge as crate::args::EnumArg>::VALUES,
                "Which of the target's edges moves. Omit it for the edge the target already owned. 'top' takes a pane's upper edge instead, so a positive amount shrinks it from the top and a negative one grows it upwards; it does nothing on the first pane, which has no edge above.",
            ),
        ],
    },
    ActionDescriptor {
        name: "resize_to",
        side: Side::Server,
        label: "Resize To",
        description: "Resize the focused column or pane to an explicit size.",
        icon: None,
        args: &[
            ArgDescriptor::required_enum(
                "target",
                <crate::input::ResizeTarget as crate::args::EnumArg>::VALUES,
                "What to resize.",
            ),
            ArgDescriptor::required("width", ArgKind::Float, "The new width."),
            ArgDescriptor::required("height", ArgKind::Float, "The new height."),
        ],
    },
    ActionDescriptor {
        name: "rename_column_to",
        side: Side::Server,
        label: "Rename Column To",
        description: "Set a column's name directly, without opening the rename prompt. An empty name clears it.",
        icon: Some(Glyph::NotePencil),
        args: &[
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the workspace holding the column.",
            ),
            ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to rename."),
            ArgDescriptor::required("name", ArgKind::Text, "The new name."),
        ],
    },
    ActionDescriptor {
        name: "rename_column_by_idx",
        side: Side::Client,
        label: "Rename Column",
        description: "Open the rename prompt for a specific column.",
        icon: Some(Glyph::NotePencil),
        args: &[
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the workspace holding the column.",
            ),
            ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to rename."),
        ],
    },
    ActionDescriptor {
        name: "take_pane",
        side: Side::Server,
        label: "Take Pane",
        description: "Pull a pane out of wherever it is and into the focused column.",
        icon: None,
        args: &[
            ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to take."),
            ArgDescriptor::optional(
                "focus_after",
                ArgKind::Bool,
                "Focus the pane once it arrives. Default false.",
            ),
        ],
    },
    ActionDescriptor {
        name: "add_column_to_workspace",
        side: Side::Server,
        label: "Add Column to Workspace",
        description: "Add a column to a specific workspace.",
        icon: Some(Glyph::FolderSimplePlus),
        args: &[ArgDescriptor::required(
            "ws_idx",
            ArgKind::Int,
            "Index of the workspace to add the column to.",
        )],
    },
    ActionDescriptor {
        name: "move_column_to_workspace",
        side: Side::Server,
        label: "Move Column to Workspace",
        description: "Move a column into another workspace.",
        icon: None,
        args: &[
            ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to move."),
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Int,
                "Index of the destination workspace.",
            ),
            ArgDescriptor::optional(
                "focus",
                ArgKind::Bool,
                "Follow the column with focus. Default true.",
            ),
        ],
    },
    ActionDescriptor {
        name: "resize_column_by",
        side: Side::Server,
        label: "Resize Column By",
        description: "Change one column's width by a fraction of the working width. The mouse divider drag and RPC use this; the keyboard resize acts on the focused column.",
        icon: None,
        args: &[
            ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to resize."),
            ArgDescriptor::required(
                "delta",
                ArgKind::Float,
                "Fraction of the working width to add; negative shrinks.",
            ),
        ],
    },
    ActionDescriptor {
        name: "resize_pane_height_by",
        side: Side::Server,
        label: "Resize Pane Height By",
        description: "Change one stacked pane's height by a number of logical pixels.",
        icon: None,
        args: &[
            ArgDescriptor::required(
                "col_idx",
                ArgKind::Int,
                "Index of the column holding the pane.",
            ),
            ArgDescriptor::required(
                "pane_idx",
                ArgKind::Int,
                "Index of the pane within the column.",
            ),
            ArgDescriptor::required(
                "delta",
                ArgKind::Float,
                "Logical pixels to add; negative shrinks.",
            ),
        ],
    },
];
