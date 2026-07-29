//! Drag surface enum dispatch.
//!
//! Routes drag operations to the correct surface handler by matching on
//! [`DragSurfaceId`]. Each function is a thin match arm that delegates to
//! the surface-specific implementation (e.g. `surface_left`).
//!
//! When a new surface is added to [`DragSurfaceId`], every function here
//! will produce a compile error until a match arm is added — this is the
//! exhaustiveness guarantee of enum dispatch.

use crate::app_state::AppState;
use crate::input::WmAction;
use heca_core::layout::PaneId;
use heca_grid_ui::drag::DragSurfaceId;

/// Returns the click action for an item in the given surface, if any.
pub(crate) fn surface_click_action(
    state: &mut AppState,
    id: DragSurfaceId,
    pos: (f32, f32),
) -> Option<WmAction> {
    match id {
        DragSurfaceId::LeftSidebar => super::surface_left::click_action(state, pos),
    }
}

/// Execute the drop: move/sway the source pane onto the target item in the given surface.
pub(crate) fn surface_accept_drop(
    state: &mut AppState,
    id: DragSurfaceId,
    pane_id: PaneId,
    original_ws: usize,
    swap: bool,
    pos: (f32, f32),
) {
    match id {
        DragSurfaceId::LeftSidebar => {
            super::surface_left::accept_drop(state, pane_id, original_ws, swap, pos)
        }
    }
}

/// Handle interactive-move drop on the given surface.
///
/// Called when a content-area drag lands on a surface. Returns `true`
/// if the drop was handled by the surface.
pub(crate) fn surface_interactive_move_drop(
    state: &mut AppState,
    id: DragSurfaceId,
    pos: (f32, f32),
) -> bool {
    match id {
        DragSurfaceId::LeftSidebar => super::surface_left::handle_interactive_move_drop(state, pos),
    }
}
