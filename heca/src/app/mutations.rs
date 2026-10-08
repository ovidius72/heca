//! App-level mutation helpers.
//!
//! These helpers coordinate multi-step pane/workspace mutations that span
//! session layout state, focus bookkeeping, backend lifecycle, and the shared
//! post-mutation hooks introduced in Phase 2.

use crate::app::focus::sync_focus;
use crate::app_state::AppState;
use heca_core::layout::{PaneId, SessionShape};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationKind {
    Layout,
    Focus,
    Config,
}

fn after_mutation_change_inner(state: &mut AppState, kind: MutationKind) {
    match kind {
        MutationKind::Layout | MutationKind::Focus | MutationKind::Config => {
            sync_focus(state);
            // **Structure — and only structure.** What makes a layer stale is a pane, column or
            // workspace appearing or going. Not the focus moving, which happens constantly and
            // would rebuild the whole map (and reset the cursor inside it) on every keystroke —
            // and, since F003/P082/T420, **not the window changing shape either**.
            //
            // `Config` used to rebuild too, because the exposé resolved a zoom from the room it had
            // when it was *built*, so an un-rebuilt layer kept the old window's zoom. That zoom is
            // gone: the map is **shares** now, and a share re-lays-out for free at any size — there
            // is nothing left for a rebuild to recompute.
            //
            // Rebuilding on resize is not merely wasted work, it is destructive: a rebuild throws
            // the widget tree away, and with it everything living *in* the tree — every hint letter
            // currently offered, and the open state of a picker the user has up. Dragging the
            // window edge with the map open therefore blanked its letters on the first frame of the
            // drag. A resize now re-lays-out what is already there.
            if matches!(kind, MutationKind::Layout) {
                refresh_visible_layers(state);
            }
            state.needs_redraw = true;
        }
    }
}

/// **A layer that is up shows the session as it is now.**
///
/// A layer's content is structural — panes open, columns and workspaces come and go — and a signal
/// replaces a prop, never a child, so staying current means being rebuilt. Until now that happened
/// only when a layer was *shown*, which is fine for opening it and wrong for everything that
/// happens while it is open: deleting a pane from the exposé removed it from the session and left
/// its card on screen, and only closing the map revealed that it had worked.
///
/// Here rather than in each handler, because "the map went stale" is not a property of any one
/// action — it is a property of the session having changed, which is exactly what this hook means.
/// Nothing happens when no host layer is visible, which is the ordinary case.
fn refresh_visible_layers(state: &mut AppState) {
    for name in state.layers.visible_host_layer_names(&state.window_root) {
        crate::chrome::rebuild_named_layer(state, &name);
    }
}

/// **What every action is followed by**, run once by the dispatcher
/// ([`ActionRegistry::execute`](crate::actions::ActionRegistry::execute)) after any handler.
///
/// Focus is re-synced and a frame is asked for, always. What lists the session — a layer that is
/// up, such as the exposé — is rebuilt only when the session's [`SessionShape`] changed: a pane,
/// column or workspace appeared, went or moved. Judged by comparing, not by the handler saying so,
/// because a handler that forgot to say so left the redraw or the map stale and nothing noticed.
pub(crate) fn after_action(state: &mut AppState, before: &SessionShape) {
    let kind = if state.session.shape() == *before {
        MutationKind::Focus
    } else {
        MutationKind::Layout
    };
    after_mutation_change_inner(state, kind);
}

pub fn after_layout_change(state: &mut AppState) {
    after_mutation_change_inner(state, MutationKind::Layout);
}

pub fn after_config_change(state: &mut AppState) {
    after_mutation_change_inner(state, MutationKind::Config);
}

pub fn after_metadata_change(state: &mut AppState) {
    after_mutation_change_inner(state, MutationKind::Focus);
}

pub fn after_mutation_change(state: &mut AppState, kind: MutationKind) {
    after_mutation_change_inner(state, kind);
}

/// Close the pane with this id, wherever it is — the one way a pane that goes away on its own (its
/// shell exited) is removed, with the same facts the close action reports.
pub(crate) fn close_pane_by_id_anywhere(state: &mut AppState, pane_id: PaneId) -> bool {
    let Some(removed) = state.layout_mut().remove_pane_anywhere(pane_id) else {
        return false;
    };
    state.apply(crate::server::Change::after_removal(removed));
    after_layout_change(state);
    true
}
