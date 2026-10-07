//! **Making room for where a pane can be put** — while a pane is carried out of the content area,
//! or a column pick is open, the layout opens a place at every gap between columns, and for the
//! rows of the column the pointer is over, as real space the columns slide apart to make.
//!
//! Whether they are open is a fact about this window's view, so it is worked out here and handed to
//! the layout read as an argument; the layout keeps nothing.

use crate::app_state::{AppState, InputMode};
use heca_core::layout::types::Point;
use heca_core::layout::{Place, PlacesOpen, RowsOpen};

/// What should be open this frame, from what is being carried and where the pointer is.
pub(crate) fn wanted(state: &AppState) -> Option<PlacesOpen> {
    let pane_area = crate::chrome::ChromeConfig::of(state).content_rect();
    let carried = heca_grid_ui::dragged_bounds(&state.window_root).is_some_and(|b| {
        pane_area.contains(Point::new(
            b.loc.x + b.size.w / 2.0,
            b.loc.y + b.size.h / 2.0,
        ))
    });
    let picking = matches!(state.input_mode, InputMode::ColumnPick { .. });
    if !carried {
        state.hovered_column.set(None);
    }
    if !carried && !picking {
        return None;
    }
    let ws = state.session.active_workspace()?;
    let scrolling = &ws.scrolling;
    let reference = scrolling
        .columns
        .get(scrolling.active_column_idx)
        .map_or(0.0, |c| c.computed_width);
    let mut open = PlacesOpen {
        column_w: reference * f64::from(state.appearance.effective_new_column_slot_share()),
        row_h: scrolling.working_area.size.h
            * f64::from(state.appearance.effective_new_row_slot_share()),
        rows: if picking { RowsOpen::All } else { RowsOpen::None },
    };
    if carried {
        // The rows of the column the carried widget is over, as the columns heard it — nothing
        // here hit-tests the pointer.
        open.rows = state
            .hovered_column
            .get()
            .and_then(|id| scrolling.columns.iter().position(|c| c.id == id))
            .map_or(RowsOpen::None, RowsOpen::Column);
    }
    Some(open)
}

/// Bring the window's layout in line with [`wanted`]; when it changes, the panes slide from where
/// they were drawn to where they are now, by the layout's own move animation.
pub(crate) fn sync(state: &mut AppState) {
    let now = wanted(state);
    if now == state.places_open {
        return;
    }
    let before = state.places_open;
    state.places_open = now;
    let Some(ws) = state.session.active_workspace_mut() else {
        return;
    };
    let was = ws.scrolling.panes_with_places_open(before);
    let is = ws.scrolling.panes_with_places_open(now);
    for (id, to) in is {
        let Some((_, from)) = was.iter().find(|(p, _)| *p == id) else {
            continue;
        };
        if let Some((col, row)) = ws.scrolling.pane_indices(id) {
            ws.scrolling.columns[col].panes[row].animate_move_from(
                Point::new(from.loc.x - to.loc.x, from.loc.y - to.loc.y),
                heca_core::layout::animation::AnimationConfig::default(),
            );
        }
    }
}

/// The open places, in window coordinates, with the box each fills.
pub(crate) fn open_places(state: &AppState) -> Vec<Place> {
    let (Some(open), Some(ws)) = (state.places_open, state.session.active_workspace()) else {
        return Vec::new();
    };
    let (dx, dy) = crate::app::terminal_host::layout_origin(state);
    ws.scrolling
        .places(open)
        .into_iter()
        .map(|mut p| {
            p.rect.loc = Point::new(p.rect.loc.x + dx, p.rect.loc.y + dy);
            p
        })
        .collect()
}
