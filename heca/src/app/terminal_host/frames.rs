//! **Where the panes are on screen**, in draw order — tiled then floating.

use crate::app_state::AppState;
use heca_core::layout::PaneId;

/// The **outer** screen rect (full pane, before content inset) of every visible
/// pane — tiled then floating — in the active workspace. Same geometry the render
/// loop derives per pane (`render.rs`); kept here so the pane-header sync step can
/// position the in-pane info bar without a GPU borrow (render's `scene_view` holds
/// `state.compositor`). Returns `(pane_id, x, y, w, h)` in logical px.
/// **The columns of the active workspace, in screen coordinates**, with the panes inside each.
///
/// The same geometry [`pane_outer_frames`] reports, grouped the way the tree is shaped. Built on
/// `ScrollingSpace::columns_with_positions`, so a caller asking about a column and a caller asking
/// about a pane read one walk.
///
/// Floating panes are **not** here: they belong to no column.
pub(crate) fn column_frames(state: &AppState) -> Vec<heca_core::layout::LaidOutColumn> {
    let pane_area = crate::chrome::ChromeConfig::of(state).content_rect();
    let ws_offset = state.layout().workspace_geometries()
        .first()
        .map(|(_, rect)| (rect.loc.x, rect.loc.y))
        .unwrap_or((0.0, 0.0));
    let (dx, dy) = (pane_area.loc.x + ws_offset.0, pane_area.loc.y + ws_offset.1);
    let Some(ws) = state.layout().active_workspace() else {
        return Vec::new();
    };
    let shift = |r: heca_core::layout::Rectangle| {
        heca_core::layout::Rectangle::new(
            heca_core::layout::types::Point::new(r.loc.x + dx, r.loc.y + dy),
            r.size,
        )
    };
    ws.scroll()
        .columns_with_positions()
        .into_iter()
        .map(|mut col| {
            col.rect = shift(col.rect);
            for pane in &mut col.panes {
                pane.rect = shift(pane.rect);
                pane.slot = shift(pane.slot);
            }
            col
        })
        .collect()
}

pub(crate) fn pane_outer_frames(state: &AppState) -> Vec<(PaneId, f32, f32, f32, f32)> {
    let pane_area = crate::chrome::ChromeConfig::of(state).content_rect();
    let ws_offset = state.layout().workspace_geometries()
        .first()
        .map(|(_, rect)| (rect.loc.x as f32, rect.loc.y as f32))
        .unwrap_or((0.0, 0.0));
    let mut frames = Vec::new();
    let Some(ws) = state.layout().active_workspace() else {
        return frames;
    };
    for (pane_id, rect) in ws.scroll().panes_with_positions() {
        let x = pane_area.loc.x as f32 + ws_offset.0 + rect.loc.x as f32;
        let y = pane_area.loc.y as f32 + ws_offset.1 + rect.loc.y as f32;
        frames.push((pane_id, x, y, rect.size.w as f32, rect.size.h as f32));
    }
    for float in &ws.floating_panes {
        let x = pane_area.loc.x as f32 + ws_offset.0 + float.position.x as f32;
        let y = pane_area.loc.y as f32 + ws_offset.1 + float.position.y as f32;
        frames.push((
            float.pane.id,
            x,
            y,
            float.size.w as f32,
            float.size.h as f32,
        ));
    }
    frames
}
