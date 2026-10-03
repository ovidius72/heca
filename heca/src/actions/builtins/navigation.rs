//! The built-in Navigation actions, in the order they are listed.

use super::ActionDescriptor;
use crate::actions::Side;
use crate::args::{ArgDescriptor, ArgKind};
use heca_grid_ui::Glyph;

pub(super) const ACTIONS: &[ActionDescriptor] = &[
    ActionDescriptor {
        name: "focus_left",
        side: Side::Client,
        label: "Focus Column Left",
        description: "Move focus to the column on the left.",
        icon: Some(Glyph::CaretLeft),
        args: &[],
    },
    ActionDescriptor {
        name: "focus_right",
        side: Side::Client,
        label: "Focus Column Right",
        description: "Move focus to the column on the right.",
        icon: Some(Glyph::CaretRight),
        args: &[],
    },
    ActionDescriptor {
        name: "focus_up",
        side: Side::Client,
        label: "Focus Pane Up",
        description: "Move focus to the pane above in the current column.",
        icon: Some(Glyph::CaretUp),
        args: &[],
    },
    ActionDescriptor {
        name: "focus_down",
        side: Side::Client,
        label: "Focus Pane Down",
        description: "Move focus to the pane below in the current column.",
        icon: Some(Glyph::CaretDown),
        args: &[],
    },
    ActionDescriptor {
        name: "next_pane",
        side: Side::Client,
        label: "Next Pane in Column",
        description: "Cycle focus forward through panes in the active column.",
        icon: Some(Glyph::ArrowLineRight),
        args: &[],
    },
    ActionDescriptor {
        name: "prev_pane",
        side: Side::Client,
        label: "Previous Pane in Column",
        description: "Cycle focus backward through panes in the active column.",
        icon: Some(Glyph::ArrowLineLeft),
        args: &[],
    },
    ActionDescriptor {
        name: "workspace_next",
        side: Side::Client,
        label: "Next Workspace",
        description: "Switch to the next workspace.",
        icon: Some(Glyph::CaretDown),
        args: &[],
    },
    ActionDescriptor {
        name: "workspace_prev",
        side: Side::Client,
        label: "Previous Workspace",
        description: "Switch to the previous workspace.",
        icon: Some(Glyph::CaretUp),
        args: &[],
    },
    ActionDescriptor {
        name: "focus_toggle_local",
        side: Side::Client,
        label: "Last Pane",
        description: "Toggle between current and last-focused pane in the same workspace.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "focus_toggle_global",
        side: Side::Client,
        label: "Last Workspace",
        description: "Toggle between current and last-visited workspace.",
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
        name: "focus_pane",
        side: Side::Client,
        label: "Focus Pane",
        description: "Move focus to a specific pane by id.",
        icon: None,
        args: &[ArgDescriptor::required(
            "pane_id",
            ArgKind::Int,
            "The pane to focus.",
        )],
    },
    ActionDescriptor {
        name: "focus_workspace",
        side: Side::Client,
        label: "Focus Workspace",
        description: "Switch to a specific workspace by index.",
        icon: None,
        args: &[ArgDescriptor::required(
            "ws_idx",
            ArgKind::Int,
            "Index of the workspace to switch to.",
        )],
    },
    ActionDescriptor {
        name: "hint_pick",
        side: Side::Client,
        label: "Pick with Letters",
        description: "Put a letter on everything you can act on; press one to act on it.",
        icon: Some(Glyph::Lightning),
        args: &[],
    },
];
