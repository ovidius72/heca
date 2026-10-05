//! The actions that run on the server: each takes what it changes (the [`ServerCx`]) and says what
//! happened, as [`Change`]s — facts, never orders to a window.
//!
//! One file per category, mirroring `crate::handlers`. An action moves here when its descriptor
//! says `Side::Server`; [`handler_for`] is the one place it is looked up.

mod add;
mod layout;
#[cfg(test)]
mod layout_tests;
mod move_column;
#[cfg(test)]
mod move_tests;
mod move_pane;
mod names;
#[cfg(test)]
mod names_tests;
mod pane;
mod remove;
#[cfg(test)]
mod remove_tests;
mod swap;
#[cfg(test)]
mod testing;

use super::{Change, ServerCx};
use crate::input::{WmAction, WmActionKind};

/// A server action's handler.
pub(crate) type ServerHandler = fn(&mut ServerCx<'_>, &WmAction) -> Vec<Change>;

/// **The server actions that cannot run on the server yet, because they start or find a terminal.**
///
/// Starting a shell builds its launch settings from the window — the event proxy that wakes the
/// loop, the theme, the cell size and viewport — and the terminals are not yet behind the server.
/// Until then these keep their window handler. The list is pinned by a test so it can only shrink,
/// and it **empties with 2b (terminals behind the server)**: a handler that starts a shell moves
/// here the day the server can start one itself.
#[cfg(test)]
pub(crate) const AWAITING_TERMINALS: &[WmActionKind] = &[
    WmActionKind::SplitHorizontal,
    WmActionKind::SplitVertical,
    WmActionKind::SpawnCommand,
    WmActionKind::CreateWorkspace,
    WmActionKind::TerminalRun,
    WmActionKind::TerminalKill,
];

/// The server handler for `kind`, if the action has been moved here.
pub(crate) fn handler_for(kind: WmActionKind) -> Option<ServerHandler> {
    use WmActionKind as K;
    Some(match kind {
        K::ResizeIncrease => layout::resize_increase,
        K::ResizeDecrease => layout::resize_decrease,
        K::ZoomColumn => layout::zoom_column,
        K::PaneHeightIncrease => layout::pane_height_increase,
        K::PaneHeightDecrease => layout::pane_height_decrease,
        K::ZoomColumnAtIndex => layout::zoom_column_at_index,
        K::Resize => layout::resize,
        K::ResizeTo => layout::resize_to,
        K::ResizeColumnBy => layout::resize_column_by,
        K::ResizePaneHeightBy => layout::resize_pane_height_by,
        K::SwapLeft => swap::swap_left,
        K::SwapRight => swap::swap_right,
        K::SwapUp => swap::swap_up,
        K::SwapDown => swap::swap_down,
        K::Swap => swap::swap,
        K::SwapColumns => swap::swap_columns,
        K::MovePaneLeft => move_pane::move_pane_left,
        K::MovePaneRight => move_pane::move_pane_right,
        K::Move => move_pane::move_to_column_here,
        K::MovePaneToWorkspace => move_pane::move_to_workspace,
        K::MovePaneToColumn => move_pane::move_to_column,
        K::MovePaneToNewColumn => move_pane::move_to_new_column,
        K::MoveColumnUp => move_column::move_column_up,
        K::MoveColumnDown => move_column::move_column_down,
        K::MoveColumnToWorkspace => move_column::move_column_to_workspace,
        K::MoveColumn => move_column::move_column,
        K::TakePane => move_pane::take_pane,
        K::PlacePane => move_pane::place_pane,
        K::AddPaneToColumn => add::add_pane_to_column,
        K::AddColumnToWorkspace => add::add_column_to_workspace,
        K::ClosePane => remove::close,
        K::ClosePaneById => remove::close_pane_by_id,
        K::DeleteColumn => remove::delete_column,
        K::DeleteWorkspace => remove::delete_workspace,
        K::RenameTarget => names::rename_target,
        K::RenameWorkspaceTo => names::rename_workspace_to,
        K::RenameColumnTo => names::rename_column_to,
        K::ResetPaneName => names::reset_pane_name,
        K::ResetPaneNameById => names::reset_pane_name_by_id,
        K::ResetWorkspaceName => names::reset_workspace_name,
        K::ResetWorkspaceNameByIdx => names::reset_workspace_name_by_idx,
        K::Float => pane::float,
        K::FloatAt => pane::float_at,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Side;
    use crate::input::resolve_action;
    use crate::server::ServerState;
    use strum::IntoEnumIterator;

    fn sides() -> crate::actions::SidesForTests {
        crate::actions::sides_for_tests()
    }

    /// **Every kind of action is on a side.** The table is built from the descriptors, so a kind no
    /// descriptor names would have no answer to "where does it run".
    #[test]
    fn every_action_kind_has_a_side() {
        let sides = sides();
        let missing: Vec<_> = WmActionKind::iter().filter(|k| sides.of(*k).is_none()).collect();
        assert!(missing.is_empty(), "kinds with no side: {missing:?}");
    }

    /// **A server action is not run by a window handler**: either the server has it, or it is on
    /// the one list of those waiting for the terminals. This is the finish line of moving the
    /// actions over: a new server action with no handler fails it, and the list can only shrink.
    #[test]
    fn every_server_action_runs_on_the_server_or_awaits_the_terminals() {
        let sides = sides();
        let stragglers: Vec<_> = WmActionKind::iter()
            .filter(|k| sides.of(*k) == Some(Side::Server))
            .filter(|k| handler_for(*k).is_none() && !AWAITING_TERMINALS.contains(k))
            .collect();
        assert!(stragglers.is_empty(), "still on the window: {stragglers:?}");
    }

    /// The waiting list can only shrink: everything on it is a server action that has no server
    /// handler yet. Once a handler is moved, it must leave the list.
    #[test]
    fn what_awaits_the_terminals_is_a_server_action_without_a_server_handler() {
        let sides = sides();
        for kind in AWAITING_TERMINALS {
            assert_eq!(sides.of(*kind), Some(Side::Server), "{kind:?}");
            assert!(handler_for(*kind).is_none(), "{kind:?} moved: take it off the list");
        }
    }

    /// **A plugin's dispatch is routed by the built-in it names.** A name-keyed action runs
    /// `cx.dispatch("zoom_column", ..)`; that is resolved by name to the same `WmAction` a key
    /// gives, and goes to its own side — the plugin's action has none.
    #[test]
    fn a_dispatch_by_name_is_routed_by_the_built_in_it_names() {
        let sides = sides();
        let zoom = resolve_action("zoom_column", &Default::default()).expect("a built-in");
        assert_eq!(sides.of(zoom.kind()), Some(Side::Server));
        assert!(ServerState::runs(zoom.kind()), "and the server has it");
        let palette = resolve_action("command_palette", &Default::default()).expect("a built-in");
        assert_eq!(sides.of(palette.kind()), Some(Side::Client));
    }
}
