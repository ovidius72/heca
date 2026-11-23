//! The built-in Workspace actions, in the order they are listed.

use super::ActionDescriptor;
use crate::actions::Side;
use crate::args::{ArgDescriptor, ArgKind};
use heca_grid_ui::Glyph;

pub(super) const ACTIONS: &[ActionDescriptor] = &[
    ActionDescriptor {
        name: "create_workspace",
        side: Side::Server,
        label: "Create Workspace",
        description: "Create a new workspace and switch to it.",
        icon: Some(Glyph::StackPlus),
        args: &[],
    },
    ActionDescriptor {
        name: "rename_workspace",
        side: Side::Client,
        label: "Rename Workspace",
        description: "Rename the current workspace.",
        icon: Some(Glyph::NotePencil),
        args: &[],
    },
    ActionDescriptor {
        name: "reset_workspace_name",
        side: Side::Server,
        label: "Use Default Name",
        description: "Clear the workspace's custom name, reverting to \"Workspace N\".",
        icon: Some(Glyph::Backspace),
        args: &[],
    },
    ActionDescriptor {
        name: "delete_current_workspace",
        side: Side::Client,
        label: "Delete Current Workspace",
        description: "Delete the workspace you are on and all its panes, after asking (not the last workspace).",
        icon: Some(Glyph::StackMinus),
        args: &[],
    },
    ActionDescriptor {
        name: "delete_workspace",
        side: Side::Server,
        label: "Delete Workspace",
        description: "Delete a workspace and all its panes (not the last workspace).",
        icon: Some(Glyph::StackMinus),
        args: &[ArgDescriptor::required(
            "ws_idx",
            ArgKind::Workspace,
            "Index of the workspace to delete.",
        )],
    },
    // ── Workspace order ──
    // What the sidebar's grip drag sends, what the workspaces dock's Shift+k/j send for the row
    // under its cursor, and what a key sends for the shown workspace.
    ActionDescriptor {
        name: "move_workspace",
        side: Side::Server,
        label: "Move Workspace To",
        description: "Move a workspace to a position in the list (counted after the move; past the end is last).",
        icon: None,
        args: &[
            ArgDescriptor::optional(
                "ws_idx",
                ArgKind::Int,
                "Index of the workspace to move; omit for the one the window shows.",
            ),
            ArgDescriptor::required(
                "to_idx",
                ArgKind::Int,
                "Where it ends up, counted as the list is after the move.",
            ),
        ],
    },
    ActionDescriptor {
        name: "move_workspace_up",
        side: Side::Server,
        label: "Move Workspace Up",
        description: "Move a workspace one place up in the list (the first stays).",
        icon: Some(Glyph::CaretUp),
        args: &[ArgDescriptor::optional(
            "ws_idx",
            ArgKind::Int,
            "Index of the workspace to move; omit for the one the window shows.",
        )],
    },
    ActionDescriptor {
        name: "move_workspace_down",
        side: Side::Server,
        label: "Move Workspace Down",
        description: "Move a workspace one place down in the list (the last stays).",
        icon: Some(Glyph::CaretDown),
        args: &[ArgDescriptor::optional(
            "ws_idx",
            ArgKind::Int,
            "Index of the workspace to move; omit for the one the window shows.",
        )],
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
        name: "rename_workspace_by_idx",
        side: Side::Client,
        label: "Rename Workspace",
        description: "Open the rename prompt for a specific workspace.",
        icon: Some(Glyph::NotePencil),
        args: &[ArgDescriptor::required(
            "ws_idx",
            ArgKind::Workspace,
            "Index of the workspace to rename.",
        )],
    },
    ActionDescriptor {
        name: "rename_workspace_to",
        side: Side::Server,
        label: "Rename Workspace To",
        description: "Set a workspace's name directly, without opening the rename prompt. An empty name clears it.",
        icon: Some(Glyph::NotePencil),
        args: &[
            ArgDescriptor::required(
                "ws_idx",
                ArgKind::Workspace,
                "Index of the workspace to rename.",
            ),
            ArgDescriptor::required("name", ArgKind::Text, "The new name."),
        ],
    },
    ActionDescriptor {
        name: "reset_workspace_name_by_idx",
        side: Side::Server,
        label: "Reset Workspace Name",
        description: "Drop a workspace's custom name so it follows its default again.",
        icon: None,
        args: &[ArgDescriptor::required(
            "ws_idx",
            ArgKind::Workspace,
            "Index of the workspace whose name to reset.",
        )],
    },
];
