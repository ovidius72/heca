//! Mouse divider-resize gesture.
//!
//! Drag the **vertical gap between columns** → resize that column's width; drag
//! the **horizontal gap between stacked panes** in a column → resize that pane's
//! height. Distinct from the DnD item-move surfaces ([`super::drag`]): this mutates
//! layout *sizes*, not pane positions.
//!
//! The gesture is intentionally registry-routed for parity: a press on a divider
//! captures a [`ResizeDrag`], and each cursor-move emits a parameterized
//! [`WmAction::ResizeColumnBy`] / [`WmAction::ResizePaneHeightBy`] (the same actions
//! RPC drives). The core resize methods mutate the **canonical** `ColumnWidth` /
//! pane `preferred_height`, so a manual resize persists through later layout
//! recomputes (the niri persistence landmine, resolved in the Phase 9 foundation).
//!
//! A **right-button-hold fallback** resizes the pane under the cursor along the
//! nearer axis, for when the thin `pane_gap` (~8px) hit-zone fights the terminal.

use crate::app_state::{AppState, ResizeDivider, ResizeDrag};
use crate::input::WmAction;
use winit::window::CursorIcon;

/// Half-width (logical px) of the grab zone on each side of a divider centerline.
/// The gap itself is `options.gaps` (~8px); this widens the catch area a little so
/// the thin divider is reachable over the terminal.
const GRAB_TOLERANCE: f32 = 6.0;

/// A laid-out tiled pane reduced to the data the divider hit-test needs:
/// its layout indices (`col`/`pane`, into the active workspace's
/// `scrolling.columns`) and its on-screen rect (logical px).
#[derive(Clone, Copy, Debug, PartialEq)]
struct PaneBox {
    col: usize,
    pane: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// Tiled (column) panes of the active workspace with layout indices + screen
/// rects. Floating panes are excluded — they aren't in any column, so
/// [`crate::find_pane_location`] returns `None` and they're filtered out.
fn pane_boxes(state: &AppState) -> Vec<PaneBox> {
    crate::app::terminal_host::pane_outer_frames(state)
        .into_iter()
        .filter_map(|(id, x, y, w, h)| {
            let (_, col, pane) = crate::find_pane_location(&state.session, id)?;
            Some(PaneBox { col, pane, x, y, w, h })
        })
        .collect()
}

/// Pure hit-test: which divider (if any) sits under `pos`. **Pane dividers are
/// checked first** — a pane's horizontal gap is a small target inside a column,
/// whereas a column's vertical gap spans the column's full height, so the inner
/// target wins ties at a T-junction.
fn divider_at(boxes: &[PaneBox], pos: (f32, f32)) -> Option<ResizeDivider> {
    let (mx, my) = pos;

    // 1) Pane dividers — the horizontal gap between vertically-stacked panes.
    for b in boxes {
        if let Some(below) = boxes
            .iter()
            .find(|o| o.col == b.col && o.pane == b.pane + 1)
        {
            let centerline = (b.y + b.h + below.y) / 2.0;
            if (my - centerline).abs() <= GRAB_TOLERANCE && mx >= b.x && mx <= b.x + b.w {
                return Some(ResizeDivider::Pane { col: b.col, pane: b.pane });
            }
        }
    }

    // 2) Column dividers — a column's **right edge** is its resize handle. Build a
    //    bounding box (min_x, max_x, min_y, max_y) per column index. Between two
    //    columns the handle spans the gap (the whole visible gap grabs); for the
    //    rightmost / single column it's a band around its right edge — which is
    //    what makes a lone or last column resizable at all.
    let mut cols: std::collections::BTreeMap<usize, (f32, f32, f32, f32)> =
        std::collections::BTreeMap::new();
    for b in boxes {
        let e = cols
            .entry(b.col)
            .or_insert((b.x, b.x + b.w, b.y, b.y + b.h));
        e.0 = e.0.min(b.x);
        e.1 = e.1.max(b.x + b.w);
        e.2 = e.2.min(b.y);
        e.3 = e.3.max(b.y + b.h);
    }
    let entries: Vec<(usize, (f32, f32, f32, f32))> = cols.into_iter().collect();
    for i in 0..entries.len() {
        let (col, (_min_x, max_x, min_y, max_y)) = entries[i];
        let (lo, hi) = if i + 1 < entries.len() {
            // Span the gap up to the next column's left edge.
            let next_min_x = entries[i + 1].1 .0;
            (max_x - GRAB_TOLERANCE, next_min_x + GRAB_TOLERANCE)
        } else {
            // Rightmost / single column: a band around its right edge.
            (max_x - GRAB_TOLERANCE, max_x + GRAB_TOLERANCE)
        };
        if mx >= lo && mx <= hi && my >= min_y && my <= max_y {
            return Some(ResizeDivider::Column { col });
        }
    }

    None
}

/// Right-button fallback: pick the divider of the pane under `pos` along the
/// **nearer** axis. Resizes the pane's own column (width) / own pane (height) —
/// works even on edge panes, since the core resize is neighbor-agnostic.
fn fallback_divider(boxes: &[PaneBox], pos: (f32, f32)) -> Option<ResizeDivider> {
    let b = boxes
        .iter()
        .copied()
        .find(|b| pos.0 >= b.x && pos.0 <= b.x + b.w && pos.1 >= b.y && pos.1 <= b.y + b.h)?;
    let multi_col = boxes.iter().any(|o| o.col != b.col);
    let multi_pane = boxes.iter().any(|o| o.col == b.col && o.pane != b.pane);
    // Distance to the nearer vertical / horizontal edge of the pane.
    let dx = (pos.0 - b.x).min(b.x + b.w - pos.0);
    let dy = (pos.1 - b.y).min(b.y + b.h - pos.1);
    match (multi_col, multi_pane) {
        (true, true) => Some(if dx <= dy {
            ResizeDivider::Column { col: b.col }
        } else {
            ResizeDivider::Pane {
                col: b.col,
                pane: b.pane,
            }
        }),
        (true, false) => Some(ResizeDivider::Column { col: b.col }),
        (false, true) => Some(ResizeDivider::Pane {
            col: b.col,
            pane: b.pane,
        }),
        (false, false) => None,
    }
}

/// Start a resize-drag if `pos` is on a divider (left-button press). Returns
/// `true` only on an actual hit — a miss falls through to the normal
/// content/drag paths so the press isn't swallowed.
pub(crate) fn on_press(state: &mut AppState, pos: (f32, f32)) -> bool {
    if !state.mouse_enabled || crate::app::interaction::is_floating_domain(&state.session) {
        return false;
    }
    let Some(divider) = divider_at(&pane_boxes(state), pos) else {
        return false;
    };
    state.mouse.resize = Some(ResizeDrag {
        divider,
        last_pos: pos,
    });
    true
}

/// Start a resize-drag from the right-button fallback (hold right-button on a
/// pane). Returns `true` if a resize was started.
pub(crate) fn on_right_press(state: &mut AppState, pos: (f32, f32)) -> bool {
    if !state.mouse_enabled || crate::app::interaction::is_floating_domain(&state.session) {
        return false;
    }
    let Some(divider) = fallback_divider(&pane_boxes(state), pos) else {
        return false;
    };
    state.mouse.resize = Some(ResizeDrag {
        divider,
        last_pos: pos,
    });
    true
}

/// Apply an incremental resize for the current drag. Returns the parameterized
/// action to dispatch (so the resize rides the registry/RPC path), or `None` when
/// not resizing / no movement. Column deltas are a fraction of the working width
/// (so the divider tracks the pointer); pane deltas are logical px.
pub(crate) fn on_drag_move(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    let drag = state.mouse.resize?;
    let (dx, dy) = (pos.0 - drag.last_pos.0, pos.1 - drag.last_pos.1);
    state.mouse.resize = Some(ResizeDrag {
        divider: drag.divider,
        last_pos: pos,
    });
    match drag.divider {
        ResizeDivider::Column { col } => {
            if dx == 0.0 {
                return None;
            }
            let working_w = state
                .session
                .active_workspace()
                .map(|ws| ws.scrolling.working_area.size.w as f32)
                .unwrap_or(0.0);
            if working_w <= 0.0 {
                return None;
            }
            Some(WmAction::ResizeColumnBy {
                col_idx: col,
                delta: (dx / working_w) as f64,
            })
        }
        ResizeDivider::Pane { col, pane } => {
            if dy == 0.0 {
                return None;
            }
            Some(WmAction::ResizePaneHeightBy {
                col_idx: col,
                pane_idx: pane,
                delta: dy as f64,
            })
        }
    }
}

/// End the resize-drag (button release). Returns `true` if one was active.
pub(crate) fn on_release(state: &mut AppState) -> bool {
    state.mouse.resize.take().is_some()
}

/// The OS cursor for the current resize context: the drag axis while resizing,
/// otherwise the axis of the divider under `pos` (for the hover affordance).
pub(crate) fn cursor_for(state: &AppState, pos: (f32, f32)) -> Option<CursorIcon> {
    let divider = match state.mouse.resize {
        Some(drag) => Some(drag.divider),
        None => divider_at(&pane_boxes(state), pos),
    }?;
    Some(match divider {
        ResizeDivider::Column { .. } => CursorIcon::ColResize,
        ResizeDivider::Pane { .. } => CursorIcon::RowResize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two columns side by side: col 0 = [0,100]x[0,200], col 1 = [108,208]x[0,200]
    // (8px gap → vertical divider centerline at x=104). Col 0 has two stacked panes
    // split at y≈96..104 (centerline y=100).
    fn layout() -> Vec<PaneBox> {
        vec![
            PaneBox { col: 0, pane: 0, x: 0.0, y: 0.0, w: 100.0, h: 96.0 },
            PaneBox { col: 0, pane: 1, x: 0.0, y: 104.0, w: 100.0, h: 96.0 },
            PaneBox { col: 1, pane: 0, x: 108.0, y: 0.0, w: 100.0, h: 200.0 },
        ]
    }

    #[test]
    fn hits_column_divider_in_the_gap() {
        assert_eq!(
            divider_at(&layout(), (104.0, 100.0)),
            Some(ResizeDivider::Column { col: 0 })
        );
    }

    #[test]
    fn hits_pane_divider_first() {
        // The pane gap centerline (col 0, y=100) wins over the column divider region.
        assert_eq!(
            divider_at(&layout(), (50.0, 100.0)),
            Some(ResizeDivider::Pane { col: 0, pane: 0 })
        );
    }

    #[test]
    fn misses_inside_pane_body() {
        assert_eq!(divider_at(&layout(), (50.0, 40.0)), None);
        assert_eq!(divider_at(&layout(), (160.0, 150.0)), None);
    }

    #[test]
    fn grab_tolerance_is_bounded() {
        // Inside col 0's gap handle [94, 114] → hits col 0.
        assert_eq!(
            divider_at(&layout(), (109.0, 50.0)),
            Some(ResizeDivider::Column { col: 0 })
        );
        // Between col 0's gap handle and col 1's right-edge handle → miss.
        assert_eq!(divider_at(&layout(), (150.0, 50.0)), None);
    }

    #[test]
    fn rightmost_column_is_resizable_at_its_right_edge() {
        // col 1 (rightmost) right edge = 208 → band [202, 214] resizes it.
        assert_eq!(
            divider_at(&layout(), (208.0, 100.0)),
            Some(ResizeDivider::Column { col: 1 })
        );
    }

    #[test]
    fn single_column_is_resizable_at_its_right_edge() {
        let one = vec![PaneBox { col: 0, pane: 0, x: 0.0, y: 0.0, w: 100.0, h: 200.0 }];
        // Right edge = 100 → band [94, 106] resizes the lone column.
        assert_eq!(
            divider_at(&one, (100.0, 100.0)),
            Some(ResizeDivider::Column { col: 0 })
        );
        // Inside the body → miss.
        assert_eq!(divider_at(&one, (50.0, 100.0)), None);
    }

    #[test]
    fn fallback_picks_nearer_axis() {
        let boxes = layout();
        // Near the right edge of col 0's top pane → column resize.
        assert_eq!(
            fallback_divider(&boxes, (98.0, 20.0)),
            Some(ResizeDivider::Column { col: 0 })
        );
        // Near the bottom edge of col 0's top pane → pane resize.
        assert_eq!(
            fallback_divider(&boxes, (50.0, 94.0)),
            Some(ResizeDivider::Pane { col: 0, pane: 0 })
        );
        // Single pane in col 1 (no pane divider) but multiple columns → column.
        assert_eq!(
            fallback_divider(&boxes, (160.0, 100.0)),
            Some(ResizeDivider::Column { col: 1 })
        );
    }
}
