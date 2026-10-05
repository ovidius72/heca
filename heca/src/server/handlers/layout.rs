//! Resizing and zooming: the layout actions that change a column or a pane and nothing else.

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// How much one key press widens or narrows the active column (a share of the working width).
const COLUMN_STEP: f64 = 0.05;
/// How much one key press makes the active pane taller or shorter, in logical pixels.
const PANE_STEP: f64 = 40.0;

/// Move the active column's right edge by `delta`.
fn resize_active_column(cx: &mut ServerCx<'_>, delta: f64) -> Vec<Change> {
    match cx.layout.active_workspace_mut() {
        Some(mut ws) => {
            ws.scroll_mut().resize_active_column(delta);
            vec![Change::LayoutChanged]
        }
        None => Vec::new(),
    }
}

pub(super) fn resize_increase(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    resize_active_column(cx, COLUMN_STEP)
}

pub(super) fn resize_decrease(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    resize_active_column(cx, -COLUMN_STEP)
}

/// Toggle the active column between viewport-wide zoom and its previous width.
pub(super) fn zoom_column(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    match cx.layout.active_workspace_mut() {
        Some(mut ws) => {
            ws.scroll_mut().toggle_active_column_zoom();
            vec![Change::LayoutChanged]
        }
        None => Vec::new(),
    }
}

fn resize_active_pane(cx: &mut ServerCx<'_>, delta: f64) -> Vec<Change> {
    match cx.layout.active_workspace_mut() {
        Some(mut ws) => {
            ws.scroll_mut().resize_active_pane_height(delta);
            vec![Change::LayoutChanged]
        }
        None => Vec::new(),
    }
}

pub(super) fn pane_height_increase(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    resize_active_pane(cx, PANE_STEP)
}

pub(super) fn pane_height_decrease(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    resize_active_pane(cx, -PANE_STEP)
}
