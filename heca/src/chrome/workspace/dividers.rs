//! **The edges between the columns and between the panes stacked in one**, worked out from the
//! rects the layout engine gave.
//!
//! Each becomes a [`Splitter`](heca_grid_ui::widgets::Splitter) child of the workspace, so dragging
//! one is the library's gesture and a float laid over it covers it.

use heca_core::layout::Rectangle;

use super::WorkspaceModel;

/// What an edge divides, by the position of what it divides: the same indices the layout engine's
/// resize takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edge {
    /// The gap right of column `col`: moves that column's width.
    Column { col: usize },
    /// The gap below pane `pane` of column `col`: moves that pane's height.
    Pane { col: usize, pane: usize },
}

impl Edge {
    /// The name the edge answers to in the workspace.
    pub(super) fn name(self) -> String {
        match self {
            Edge::Column { col } => format!("split:col:{col}"),
            Edge::Pane { col, pane } => format!("split:pane:{col}:{pane}"),
        }
    }
}

/// Every edge, with the window rect of the gap it sits in.
pub(super) fn edges(model: &WorkspaceModel) -> Vec<(Edge, Rectangle)> {
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        Rectangle::new(
            heca_core::layout::Point::new(x as f64, y as f64),
            heca_core::layout::Size::new(w.max(0.0) as f64, h as f64),
        )
    };
    let mut out = Vec::new();
    for (col, pair) in model.columns.windows(2).enumerate() {
        let (left, right) = (&pair[0], &pair[1]);
        let from = left.x + left.w;
        out.push((
            Edge::Column { col },
            rect(from, left.y, right.x - from, left.h),
        ));
    }
    for (col, column) in model.columns.iter().enumerate() {
        for (pane, pair) in column.panes.windows(2).enumerate() {
            let (above, below) = (&pair[0], &pair[1]);
            let from = above.y + above.h;
            out.push((
                Edge::Pane { col, pane },
                rect(above.x, from, above.w, below.y - from),
            ));
        }
    }
    out
}
