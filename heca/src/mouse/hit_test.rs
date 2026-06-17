//! Mouse hit-testing helpers.
//!
//! This module isolates pane and sidebar item hit detection from the broader
//! drag / click / drop state machine.

use crate::app_state::AppState;
use heca_core::layout::PaneId;

/// Find which pane (if any) is under the cursor.
/// Floating panes are tested before scrolling panes.
pub(crate) fn hit_test_pane(state: &AppState, pos: (f32, f32)) -> Option<PaneId> {
    hit_test_pane_excluding(state, pos, None)
}

/// Find which pane (if any) is under the cursor, optionally excluding one pane ID.
/// This is used during swap-drag to skip the dragged source pane.
pub(crate) fn hit_test_pane_excluding(state: &AppState, pos: (f32, f32), exclude: Option<PaneId>) -> Option<PaneId> {
    let (win_w, win_h) = super::window_logical_size(state);
    let chrome = super::chrome_config(state);
    let pane_area = chrome.content_rect(win_w, win_h);

    let sidebar_left_w = if state.chrome_state.left_visible() {
        chrome.left_sidebar_width
    } else {
        40.0
    };
    if pos.0 < sidebar_left_w {
        return None;
    }
    if pos.1 < chrome.tab_bar_height || pos.1 > win_h - chrome.status_bar_height {
        return None;
    }
    if pos.0 > win_h / 2.0
        && state.chrome_state.right_visible()
        && pos.0 > win_w - chrome.right_sidebar_width
    {
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
            let fx = pane_area.loc.x as f32 + ws_offset.0 + float.position.x as f32;
            let fy = pane_area.loc.y as f32 + ws_offset.1 + float.position.y as f32;
            let fw = float.size.w as f32;
            let fh = float.size.h as f32;
            if pos.0 >= fx && pos.0 < fx + fw && pos.1 >= fy && pos.1 < fy + fh
                && Some(float.pane.id) != exclude
            {
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
        let px = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let py = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        let pw = rect.size.w as f32;
        let ph = rect.size.h as f32;
        if pos.0 >= px && pos.0 < px + pw && pos.1 >= py && pos.1 < py + ph
            && Some(*pane_id) != exclude
        {
            return Some(*pane_id);
        }
    }

    None
}

