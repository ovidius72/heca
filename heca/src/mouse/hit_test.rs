//! Mouse hit-testing helpers.
//!
//! This module isolates pane and sidebar item hit detection from the broader
//! drag / click / drop state machine.

use crate::app_state::AppState;
use heca_core::layout::PaneId;
use heca_core::layout::types::{Point, Rectangle};

/// Find which pane (if any) is under the cursor.
/// Floating panes are tested before scrolling panes.
pub(crate) fn hit_test_pane(state: &AppState, pos: (f32, f32)) -> Option<PaneId> {
    hit_test_pane_excluding(state, pos, None)
}

/// Find which pane (if any) is under the cursor, optionally excluding one pane ID.
/// This is used during swap-drag to skip the dragged source pane.
pub(crate) fn hit_test_pane_excluding(
    state: &AppState,
    pos: (f32, f32),
    exclude: Option<PaneId>,
) -> Option<PaneId> {
    let chrome = crate::chrome::ChromeConfig::of(state);
    let pane_area = chrome.content_rect();

    // **The pane area answers where it is.** Every edge this used to test by hand — the sidebar on
    // each side, the tab bar above, the status bar below — is already `content_rect`, so asking it
    // is one question instead of four copies that can disagree with the thing they describe. Two
    // did: a hidden left sidebar left a 40px strip nothing could be clicked in (a rail's width,
    // for a rail that was deleted), and the right edge was guarded by an **x** compared against
    // half the window's **height**.
    if !pane_area.contains(at(pos)) {
        return None;
    }

    let ws_offset = state
        .session
        .workspace_geometries()
        .first()
        .map(|(_, r)| (r.loc.x as f32, r.loc.y as f32))
        .unwrap_or((0.0, 0.0));

    if let Some(ws) = state.session.active_workspace() {
        for float in &ws.floating_panes {
            let seat = placed(pane_area, ws_offset, float.position, float.size);
            if seat.contains(at(pos)) && Some(float.pane.id) != exclude {
                return Some(float.pane.id);
            }
        }
    }

    let pane_positions = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.panes_with_positions())
        .unwrap_or_default();

    for (pane_id, rect) in &pane_positions {
        let seat = placed(pane_area, ws_offset, rect.loc, rect.size);
        if seat.contains(at(pos)) && Some(*pane_id) != exclude {
            return Some(*pane_id);
        }
    }

    None
}

/// The cursor as a point the layout types can answer about.
fn at(pos: (f32, f32)) -> Point {
    Point::new(pos.0 as f64, pos.1 as f64)
}

/// **Where a pane actually sits on the glass**: its place within the workspace, offset by where the
/// workspace sits in the pane area. One spelling for a float and a tiled pane alike — they differ
/// in where their rectangle comes from, never in how it is put on screen.
fn placed(
    pane_area: Rectangle,
    ws_offset: (f32, f32),
    loc: heca_core::layout::types::Point,
    size: heca_core::layout::types::Size,
) -> Rectangle {
    Rectangle::new(
        Point::new(
            pane_area.loc.x + ws_offset.0 as f64 + loc.x,
            pane_area.loc.y + ws_offset.1 as f64 + loc.y,
        ),
        size,
    )
}
