//! Floating a pane and putting it back.

use heca_core::layout::{ColumnId, Rectangle};
use heca_core::layout::types::{Point, Size};

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// Toggle the asking window's focused pane between floating and tiled.
///
/// A floating pane goes back where it came from; a tiled one floats centred at the layout's float
/// size. The column id is allocated before the workspace is borrowed and spent only if putting the
/// pane back has to rebuild its column — a derived id could collide with one that still exists.
pub(super) fn float(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let Some(pane) = cx.asker.focused_pane else {
        return Vec::new();
    };
    let new_column_id = ColumnId(cx.layout.next_id());
    let Some(mut ws) = cx.layout.active_workspace_mut() else {
        return Vec::new();
    };
    if !ws.unfloat_pane(pane, new_column_id) {
        let rect = ws.default_float_rect();
        ws.float_tiled_pane(pane, rect);
    }
    vec![Change::LayoutChanged]
}

/// Float a pane at an exact place.
pub(super) fn float_at(cx: &mut ServerCx<'_>, action: &WmAction) -> Vec<Change> {
    let WmAction::FloatAt {
        pane_id,
        x,
        y,
        width,
        height,
    } = action
    else {
        return Vec::new();
    };
    let rect = Rectangle::new(Point::new(*x, *y), Size::new(*width, *height));
    cx.change_active_workspace(|mut ws| {
        ws.float_tiled_pane(*pane_id, rect);
    })
}
