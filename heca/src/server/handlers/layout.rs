//! Resizing and zooming: the layout actions that change a column or a pane and nothing else.

use heca_core::layout::ColumnWidth;

use crate::input::{ResizeEdge, ResizeTarget, WmAction};
use crate::server::{Change, ServerCx};

/// How much one key press widens or narrows the active column (a share of the working width).
const COLUMN_STEP: f64 = 0.05;
/// How much one key press makes the active pane taller or shorter, in logical pixels.
const PANE_STEP: f64 = 40.0;

pub(super) fn resize_increase(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| ws.scroll_mut().resize_active_column(COLUMN_STEP))
}

pub(super) fn resize_decrease(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| ws.scroll_mut().resize_active_column(-COLUMN_STEP))
}

/// Toggle the active column between viewport-wide zoom and its previous width.
pub(super) fn zoom_column(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| {
        ws.scroll_mut().toggle_active_column_zoom();
    })
}

pub(super) fn pane_height_increase(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| ws.scroll_mut().resize_active_pane_height(PANE_STEP))
}

pub(super) fn pane_height_decrease(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_workspace(|mut ws| ws.scroll_mut().resize_active_pane_height(-PANE_STEP))
}

/// Zoom the column at `(ws_idx, col_idx)` — in that workspace, whichever one a window is showing.
/// Which workspace a window shows, and which column it has active, are the window's to decide
/// from the [`Change::ColumnZoomed`] this reports.
pub(super) fn zoom_column_at_index(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ZoomColumnAtIndex { ws_idx, col_idx } = action else {
        return Vec::new();
    };
    let Some(mut ws) = cx.layout.workspace_mut(*ws_idx) else {
        return Vec::new();
    };
    match ws.scroll_mut().toggle_column_zoom(*col_idx) {
        true => vec![Change::ColumnZoomed {
            workspace: *ws_idx,
            column: *col_idx,
        }],
        false => Vec::new(),
    }
}

/// Resize the active column across, or the active pane down, by `amount`.
///
/// The target decides the axis: a column is resized across, a pane down. A key that quietly does
/// nothing is worse than one that is refused, so there is no separate axis to disagree with it.
pub(super) fn resize(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::Resize {
        target,
        amount,
        edge,
    } = action
    else {
        return Vec::new();
    };
    cx.change_asker_workspace(|mut ws| match target {
        ResizeTarget::Column => {
            let delta = *amount / 1000.0;
            // A column's left edge is the right edge of the column before it, so `Left` moves
            // that boundary; the scrolling space owns what each edge means.
            match edge {
                ResizeEdge::Left => ws.scroll_mut().move_active_column_left_boundary(delta),
                _ => ws.scroll_mut().resize_active_column(delta),
            }
        }
        ResizeTarget::Pane => {
            let height = ws.scroll().area().size.h;
            let gaps = ws.scrolling.options.gaps;
            if let Some(col) = ws.scroll_mut().active_column_mut() {
                // A boundary and a direction, not "grow me": positive is down, whichever pane is
                // active. `Auto` is the edge the pane already owned.
                match edge {
                    ResizeEdge::Top => col.move_active_pane_top_boundary(*amount, height, gaps),
                    _ => col.move_active_pane_boundary(*amount, height, gaps),
                }
            }
        }
    })
}

/// Give the active column a fixed width, or the active pane a height.
pub(super) fn resize_to(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ResizeTo {
        target,
        width,
        height,
    } = action
    else {
        return Vec::new();
    };
    cx.change_asker_workspace(|mut ws| match target {
        ResizeTarget::Column => {
            if let Some(col) = ws.scroll_mut().active_column_mut() {
                col.width = ColumnWidth::Fixed(*width);
            }
        }
        ResizeTarget::Pane => {
            if let Some(col) = ws.scroll_mut().active_column_mut() {
                let pane_idx = col.active_pane_idx;
                if let Some(pane) = col.panes.get_mut(pane_idx) {
                    pane.preferred_height = Some(*height);
                }
            }
        }
    })
}

/// Resize one column's width — a divider drag or RPC. `delta` is a proportion delta (or a fraction
/// of the working width for a fixed column), so a pixel drag maps as `dx / working_area.width`.
pub(super) fn resize_column_by(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ResizeColumnBy { col_idx, delta } = action else {
        return Vec::new();
    };
    cx.change_asker_workspace(|mut ws| ws.scroll_mut().resize_column(*col_idx, *delta))
}

/// Resize one stacked pane's height — a divider drag or RPC. `delta` is logical px; dragging down
/// makes it taller.
pub(super) fn resize_pane_height_by(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ResizePaneHeightBy {
        col_idx,
        pane_idx,
        delta,
    } = action
    else {
        return Vec::new();
    };
    cx.change_asker_workspace(|mut ws| {
        ws.scroll_mut().resize_pane_height(*col_idx, *pane_idx, *delta)
    })
}
