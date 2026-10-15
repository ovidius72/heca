//! **Whether the places a pane can be put are shown** — while a pane is carried, or a column pick
//! is open. A fact about this window's view, so it lives here and not in the layout, which only
//! reports where each place is.

use crate::app_state::{AppState, InputMode};
use heca_core::layout::Place;
use heca_core::layout::types::Point;

/// Whether the places should be shown this frame: a pane is being carried — from a column, a
/// sidebar or anywhere else that offers one, as the drag says — or a column pick is open.
pub(crate) fn wanted(state: &AppState) -> bool {
    let carried = heca_grid_ui::dragged_kind(&state.window_root).as_deref()
        == Some(crate::chrome::PANE_DRAG_KIND);
    carried || matches!(state.input_mode, InputMode::ColumnPick { .. })
}

/// Bring the window's view in line with [`wanted`].
pub(crate) fn sync(state: &mut AppState) {
    state.places_open = wanted(state);
}

/// The open places, in window coordinates, with the box each fills.
pub(crate) fn open_places(state: &AppState) -> Vec<Place> {
    let (true, Some(ws)) = (state.places_open, state.layout().active_workspace()) else {
        return Vec::new();
    };
    let (dx, dy) = crate::app::terminal_host::layout_origin(state);
    ws.scroll()
        .places()
        .into_iter()
        .map(|p| p.moved_by(Point::new(dx, dy)))
        .collect()
}
