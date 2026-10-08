//! **Whether the places a pane can be put are shown** — while a pane is carried out of the content
//! area, or a column pick is open. A fact about this window's view, so it lives here and not in the
//! layout, which only reports where each place is.

use crate::app_state::{AppState, InputMode};
use heca_core::layout::types::Point;
use heca_core::layout::Place;

/// Whether the places should be shown this frame: a pane is being carried out of the content area,
/// or a column pick is open.
pub(crate) fn wanted(state: &AppState) -> bool {
    let pane_area = crate::chrome::ChromeConfig::of(state).content_rect();
    // Once on it stays on for as long as the drag lasts, so it does not decide again from where
    // the carried pane happens to sit.
    let carried = heca_grid_ui::dragging(&state.window_root)
        && (state.places_open
            || heca_grid_ui::dragged_bounds(&state.window_root).is_some_and(|b| {
                pane_area.contains(Point::new(
                    b.loc.x + b.size.w / 2.0,
                    b.loc.y + b.size.h / 2.0,
                ))
            }));
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
        .map(|mut p| {
            p.rect.loc = Point::new(p.rect.loc.x + dx, p.rect.loc.y + dy);
            p
        })
        .collect()
}
