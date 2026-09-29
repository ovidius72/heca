//! The built-in Workspace actions, in the order they are listed.

use super::ActionDescriptor;
use crate::args::{ArgDescriptor, ArgKind};
use heca_grid_ui::Glyph;

pub(super) const ACTIONS: &[ActionDescriptor] = &[
    ActionDescriptor {
        name: "create_workspace",
        label: "Create Workspace",
        description: "Create a new workspace and switch to it.",
        icon: Some(Glyph::StackPlus),
        args: &[],
    },
    ActionDescriptor {
        name: "rename_workspace",
        label: "Rename Workspace",
        description: "Rename the current workspace.",
        icon: Some(Glyph::NotePencil),
        args: &[],
    },
    ActionDescriptor {
        name: "reset_workspace_name",
        label: "Use Default Name",
        description: "Clear the workspace's custom name, reverting to \"Workspace N\".",
        icon: Some(Glyph::Backspace),
        args: &[],
    },
    ActionDescriptor {
        name: "delete_workspace",
        label: "Delete Workspace",
        description: "Delete a workspace and all its panes (not the last workspace).",
        icon: Some(Glyph::StackMinus),
        args: &[ArgDescriptor::required(
            "ws_idx",
            ArgKind::Int,
            "Index of the workspace to delete.",
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
        label: "Rename Workspace",
        description: "Open the rename prompt for a specific workspace.",
        icon: Some(Glyph::NotePencil),
        args: &[ArgDescriptor::required(
            "ws_idx",
            ArgKind::Int,
            "Index of the workspace to rename.",
        )],
    },
    ActionDescriptor {
        name: "reset_workspace_name_by_idx",
        label: "Reset Workspace Name",
        description: "Drop a workspace's custom name so it follows its default again.",
        icon: None,
        args: &[ArgDescriptor::required(
            "ws_idx",
            ArgKind::Int,
            "Index of the workspace whose name to reset.",
        )],
    },
];
