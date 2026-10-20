//! Floating a pane and putting it back.

use heca_core::layout::types::{Point, Size};
use heca_core::layout::{ColumnId, PaneId, Rectangle};

use crate::input::WmAction;
use crate::server::{Change, ServerCx};

/// Toggle the asking window's focused pane between floating and tiled.
///
/// A floating pane goes back where it came from, and the asker focuses it there; a tiled one floats
/// in the middle of the asker's area, at the layout's float size. The column id is allocated before
/// the workspace is borrowed and spent only if putting the pane back has to rebuild its column — a
/// derived id could collide with one that still exists.
pub(super) fn float(cx: &mut ServerCx<'_>, _action: &WmAction) -> Vec<Change> {
    let Some(pane) = cx.asker.focused_pane else {
        return Vec::new();
    };
    let new_column_id = ColumnId(cx.layout.next_id());
    let (workspace, area) = (cx.asker.workspace, cx.asker.area);
    let Some(ws) = cx.layout.workspaces.get_mut(workspace) else {
        return Vec::new();
    };
    if let Some(effect) = ws.unfloat_pane(pane, new_column_id, area.size.h) {
        let column = ws.scrolling.pane_indices(pane).map_or(0, |(col, _)| col);
        let landed = Change::PaneMoved {
            pane,
            workspace,
            column,
        };
        return vec![
            Change::Arranged {
                workspace,
                effects: vec![effect],
            },
            landed,
        ];
    }
    let rect = ws.float_rect_in(area);
    float_tiled(cx, pane, rect)
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
    float_tiled(cx, *pane_id, rect)
}

/// Float the tiled pane `pane` of the asker's workspace, drawn at `rect` in the asking window.
///
/// Two facts: what leaving its column did to the strip (content, shown by every window) and that
/// it now floats at `rect` (the asking window places and focuses it; another window falls back to
/// its own centred default).
fn float_tiled(cx: &mut ServerCx<'_>, pane: PaneId, rect: Rectangle) -> Vec<Change> {
    let workspace = cx.asker.workspace;
    let Some(effect) = cx
        .layout
        .workspaces
        .get_mut(workspace)
        .and_then(|ws| ws.float_tiled_pane(pane))
    else {
        return Vec::new();
    };
    vec![
        Change::Arranged {
            workspace,
            effects: vec![effect],
        },
        Change::Floated {
            pane,
            workspace,
            rect,
        },
    ]
}
