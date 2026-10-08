//! Resizing and zooming: the layout actions that change a column or a pane and nothing else.

use heca_core::layout::ColumnWidth;

use crate::input::{ResizeEdge, ResizeTarget, WmAction};
use crate::server::{Change, ServerCx};

/// How much one key press widens or narrows the active column (a share of the working width).
const COLUMN_STEP: f64 = 0.05;
/// How much one key press makes the active pane taller or shorter, in logical pixels.
const PANE_STEP: f64 = 40.0;

pub(super) fn resize_increase(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_columns(|space, asker| {
        space.resize_column_by(asker.column, COLUMN_STEP, asker.area.w).into_iter().collect()
    })
}

pub(super) fn resize_decrease(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_columns(|space, asker| {
        space.resize_column_by(asker.column, -COLUMN_STEP, asker.area.w).into_iter().collect()
    })
}

/// Toggle the active column between viewport-wide zoom and its previous width.
pub(super) fn zoom_column(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    cx.change_asker_columns(|space, asker| space.zoom_column(asker.column).into_iter().collect())
}

/// Give the asker's active pane `delta` more px of height (less when negative).
fn resize_active_pane(cx: &mut ServerCx<'_>, delta: f64) -> Vec<Change> {
    cx.change_asker_content(|space, asker| {
        let gaps = space.options.gaps;
        if let Some(column) = space.columns.get_mut(asker.column) {
            column.resize_active_pane_height(delta, asker.area.h, gaps);
        }
    })
}

pub(super) fn pane_height_increase(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    resize_active_pane(cx, PANE_STEP)
}

pub(super) fn pane_height_decrease(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    resize_active_pane(cx, -PANE_STEP)
}

/// Zoom the column at `(ws_idx, col_idx)` — in that workspace, whichever one a window is showing.
/// Which workspace a window shows, and which column it has active, are the window's to decide
/// from the [`Change::ColumnZoomed`] this reports.
pub(super) fn zoom_column_at_index(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ZoomColumnAtIndex { ws_idx, col_idx } = action else {
        return Vec::new();
    };
    let Some(space) = cx.layout.columns_mut(*ws_idx) else {
        return Vec::new();
    };
    match space.zoom_column(*col_idx) {
        Some(_) => vec![Change::ColumnZoomed {
            workspace: *ws_idx,
            column: *col_idx,
        }],
        None => Vec::new(),
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
    match target {
        ResizeTarget::Column => cx.change_asker_columns(|space, asker| {
            let delta = *amount / 1000.0;
            // A column's left edge is the right edge of the column before it, so `Left` moves
            // that boundary; the scrolling space owns what each edge means.
            match edge {
                ResizeEdge::Left => space.move_left_boundary(asker.column, delta, asker.area.w),
                _ => space.resize_column_by(asker.column, delta, asker.area.w).into_iter().collect(),
            }
        }),
        ResizeTarget::Pane => cx.change_asker_content(|space, asker| {
            let gaps = space.options.gaps;
            if let Some(col) = space.columns.get_mut(asker.column) {
                // A boundary and a direction, not "grow me": positive is down, whichever pane is
                // active. `Auto` is the edge the pane already owned.
                match edge {
                    ResizeEdge::Top => col.move_active_pane_top_boundary(*amount, asker.area.h, gaps),
                    _ => col.move_active_pane_boundary(*amount, asker.area.h, gaps),
                }
            }
        }),
    }
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
    match target {
        ResizeTarget::Column => cx.change_asker_columns(|space, asker| {
            space
                .set_column_width(asker.column, ColumnWidth::Fixed(*width))
                .into_iter()
                .collect()
        }),
        ResizeTarget::Pane => cx.change_asker_content(|space, asker| {
            let gaps = space.options.gaps;
            if let Some(col) = space.columns.get_mut(asker.column) {
                col.set_pane_height(col.active_pane_idx, *height, asker.area.h, gaps);
            }
        }),
    }
}

/// Resize one column's width — a divider drag or RPC. `delta` is a proportion delta (or a fraction
/// of the working width for a fixed column), so a pixel drag maps as `dx / working_area.width`.
pub(super) fn resize_column_by(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::ResizeColumnBy { col_idx, delta } = action else {
        return Vec::new();
    };
    cx.change_asker_columns(|space, asker| {
        space.resize_column_by(*col_idx, *delta, asker.area.w).into_iter().collect()
    })
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
    cx.change_asker_content(|space, asker| {
        let gaps = space.options.gaps;
        if let Some(col) = space.columns.get_mut(*col_idx) {
            col.resize_pane_height(*pane_idx, *delta, asker.area.h, gaps);
        }
    })
}
