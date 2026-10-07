//! The modes that wait for one letter — pick a pane, a link, a hint, a column, a dock.

use super::KeyInputContext;
use crate::actions::ActionRegistry;
use crate::app::interaction::InteractionSource;
use crate::app::interaction::dispatch_action;
use crate::app::keyboard::typed_candidate_char;
use crate::app::selection::find_pane_location;
use crate::app_state::{AppState, InputMode, WorkspacePickTarget};
use crate::input::WmAction;
use heca_core::layout::PaneId;

pub(super) fn handle_pane_select_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, PaneId)],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::FocusPane {
                pane_id: *target_id,
            },
        );
    }
    state.input_mode = InputMode::Normal;
}

pub(super) fn handle_follow_link_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[crate::app_state::LinkHint],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    // Any key exits the overlay; a matching letter opens its link. Esc just exits.
    state.input_mode = InputMode::Normal;
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some(hint) = candidates.iter().find(|h| h.label == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::OpenLink {
                url: hint.url.clone(),
            },
        );
    }
    state.needs_redraw = true;
}

/// Universal picker (`InputMode::HintPick`): a matching letter runs what the picked region said a
/// pick does to it; any other key / Esc just exits. Mirrors [`handle_follow_link_mode`].
///
/// **The host resolves nothing.** There is no registry to look an id up in and no intent to route
/// here: the region declared the behaviour itself (`KeyHint::on_hint`, or a described node's `hint`
/// event), and running it emits whatever that region emits — which is how a plugin's row gets the
/// same picker the app's own rows have. A target whose tree was rebuilt under the letters simply
/// answers `false`.
pub(super) fn handle_hint_pick_mode(
    _registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, crate::chrome::HintTarget)],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;
    // The letters come down however this ends — picked, wrong key, or Esc. Withdrawn before the
    // pick runs, because running it may tear the tree down and a keycap must not outlive the mode
    // that put it up.
    crate::chrome::clear_hint_letters(state);
    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, target)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        crate::chrome::fire_hint(state, target);
    }
    state.needs_redraw = true;
}

pub(super) fn handle_pane_swap_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, PaneId)],
    focus_after: bool,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    let current_id = state.focused_pane;
    if let Some(ch) = typed
        && let Some(current_id) = current_id
        && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
        && let Some((_, _, _)) = find_pane_location(&state.session, current_id)
        && let Some((_, _, _)) = find_pane_location(&state.session, *target_id)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::Swap {
                a_id: current_id,
                b_id: *target_id,
            },
        );

        if focus_after {
            dispatch_action(
                state,
                registry,
                InteractionSource::Keyboard,
                &WmAction::FocusPane {
                    pane_id: current_id,
                },
            );
        } else {
            dispatch_action(
                state,
                registry,
                InteractionSource::Keyboard,
                &WmAction::FocusPane {
                    pane_id: *target_id,
                },
            );
        }
    }
    state.needs_redraw = true;
}

pub(super) fn handle_pane_take_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, PaneId)],
    focus_after: bool,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, target_id)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::TakePane {
                pane_id: *target_id,
                focus_after,
            },
        );
    }
    state.needs_redraw = true;
}

/// Resolve a [`InputMode::WorkspacePick`] keypress: a matching candidate letter moves
/// the captured `target` (active column or pane) into that workspace; any other key
/// (e.g. Esc) just exits the mode. Mirrors [`handle_pane_swap_mode`].
pub(super) fn handle_workspace_pick_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, usize, heca_core::layout::WorkspaceId)],
    target: WorkspacePickTarget,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, target_ws, _)) = candidates.iter().find(|(c, ..)| *c == ch)
    {
        let target_ws = *target_ws;
        let action = match target {
            WorkspacePickTarget::Column {
                ws_idx: origin_ws,
                col_idx,
            } => {
                // `move_column_to_workspace` resolves the source column against the
                // active workspace, so re-activate the captured origin first in case
                // the active workspace drifted while the pick was open.
                if state.session.active_workspace_idx != origin_ws {
                    crate::app::focus::switch_workspace_tracked(state, origin_ws);
                }
                WmAction::MoveColumnToWorkspace {
                    col_idx,
                    ws_idx: target_ws,
                    focus: true,
                }
            }
            WorkspacePickTarget::Pane(pane_id) => WmAction::MovePaneToWorkspace {
                pane_id,
                ws_idx: target_ws,
            },
        };
        dispatch_action(state, registry, InteractionSource::Keyboard, &action);
    }
    state.needs_redraw = true;
}

/// **What picking a destination does to the captured pane**: stack it into a column that exists, or
/// make a column of its own at a gap of the active workspace (counted with the pane where it is).
fn column_pick_action(
    target: crate::app_state::ColumnPickTarget,
    pane_id: PaneId,
    active_ws: usize,
) -> WmAction {
    use crate::app_state::ColumnPickTarget;
    match target {
        ColumnPickTarget::Existing {
            ws_idx, col_idx, ..
        } => WmAction::MovePaneToColumn {
            pane_id,
            ws_idx,
            col_idx,
        },
        ColumnPickTarget::NewColumn { gap } => WmAction::PlacePane {
            pane_id,
            ws_idx: active_ws,
            col_idx: gap,
            pane_idx: None,
        },
        ColumnPickTarget::NewRow { col, row } => WmAction::PlacePane {
            pane_id,
            ws_idx: active_ws,
            col_idx: col,
            pane_idx: Some(row),
        },
    }
}

/// Resolve a [`InputMode::ColumnPick`] keypress: a matching candidate letter moves the
/// captured pane into that column of the active workspace (stacking with its panes);
/// any other key (e.g. Esc) exits the mode.
pub(super) fn handle_column_pick_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, crate::app_state::ColumnPickTarget)],
    pane_id: PaneId,
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, target)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        // Each destination names the act that reaches it, so the letter runs the same action a
        // keybinding or an RPC call would — the pick is only how a keyboard supplies an argument it
        // cannot type.
        let action = column_pick_action(*target, pane_id, state.session.active_workspace_idx);
        dispatch_action(state, registry, InteractionSource::Keyboard, &action);
    }
    state.needs_redraw = true;
}

/// Resolve a [`InputMode::DockPick`] keypress: a matching candidate letter gives that **dock**
/// chrome keyboard focus; any other key (e.g. Esc) exits the mode.
///
/// It goes back out through the same `focus_dock` action, carrying the picked id — so the letter, an
/// RPC call and a script all take one path, and the pick is only how a keyboard supplies an argument
/// it cannot type (F003/P011/T020).
pub(super) fn handle_dock_pick_mode(
    registry: &ActionRegistry,
    state: &mut AppState,
    candidates: &[(char, crate::chrome::ContainerId)],
    ctx: KeyInputContext<'_>,
) {
    let candidates = candidates.to_vec();
    state.input_mode = InputMode::Normal;

    let typed = typed_candidate_char(ctx.key_text, ctx.physical_key, ctx.is_shift);
    if let Some(ch) = typed
        && let Some((_, dock)) = candidates.iter().find(|(c, _)| *c == ch)
    {
        dispatch_action(
            state,
            registry,
            InteractionSource::Keyboard,
            &WmAction::FocusDock {
                dock: Some(dock.clone()),
            },
        );
    }
    state.needs_redraw = true;
}

#[cfg(test)]
mod column_pick_tests {
    use super::*;
    use crate::app_state::ColumnPickTarget;
    use heca_core::layout::ColumnId;

    /// A letter on a place for a new column makes the column at that gap of the workspace on screen;
    /// a letter on a column stacks the pane into it.
    #[test]
    fn a_place_makes_a_column_at_its_gap_and_a_column_takes_the_pane() {
        assert_eq!(
            column_pick_action(ColumnPickTarget::NewColumn { gap: 2 }, PaneId(5), 1),
            WmAction::PlacePane {
                pane_id: PaneId(5),
                ws_idx: 1,
                col_idx: 2,
                pane_idx: None,
            }
        );
        assert_eq!(
            column_pick_action(ColumnPickTarget::NewRow { col: 1, row: 2 }, PaneId(5), 1),
            WmAction::PlacePane {
                pane_id: PaneId(5),
                ws_idx: 1,
                col_idx: 1,
                pane_idx: Some(2),
            }
        );
        assert_eq!(
            column_pick_action(
                ColumnPickTarget::Existing {
                    ws_idx: 0,
                    col_idx: 3,
                    col_id: ColumnId(9),
                },
                PaneId(5),
                1
            ),
            WmAction::MovePaneToColumn {
                pane_id: PaneId(5),
                ws_idx: 0,
                col_idx: 3,
            }
        );
    }
}
