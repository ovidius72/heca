//! **The edges of the columns and between the panes stacked in one**, worked out from the rects the
//! layout engine gave: between two columns, and also the outer edges of the first and last, so a
//! lone column and the ones at either end can be resized too.
//!
//! Each becomes a [`Splitter`](heca_grid_ui::widgets::Splitter) child of the workspace, so dragging
//! one is the library's gesture and a float laid over it covers it.

use heca_core::layout::Rectangle;

use super::WorkspaceModel;

/// What an edge divides, by the position of what it divides: the same indices the layout engine's
/// resize takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edge {
    /// The gap right of column `col` — or, for the last, its right border: moves that column's
    /// width.
    Column { col: usize },
    /// The left border of column `col`, the first: moves that column's width, wider to the left.
    ColumnStart { col: usize },
    /// The gap below pane `pane` of column `col`: moves that pane's height.
    Pane { col: usize, pane: usize },
}

impl Edge {
    /// The name the edge answers to in the workspace.
    pub(super) fn name(self) -> String {
        match self {
            Edge::Column { col } => format!("split:col:{col}"),
            Edge::ColumnStart { col } => format!("split:colstart:{col}"),
            Edge::Pane { col, pane } => format!("split:pane:{col}:{pane}"),
        }
    }
}

/// Every edge, with the window rect of the gap it sits in. **None while a pane floats**: nothing is
/// resized then, so there is no edge to grab and no resize cursor to show.
pub(super) fn edges(model: &WorkspaceModel) -> Vec<(Edge, Rectangle)> {
    if !model.floats.is_empty() {
        return Vec::new();
    }
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        Rectangle::new(
            heca_core::layout::Point::new(x as f64, y as f64),
            heca_core::layout::Size::new(w.max(0.0) as f64, h as f64),
        )
    };
    let mut out = Vec::new();
    if let Some(first) = model.columns.first() {
        out.push((
            Edge::ColumnStart { col: 0 },
            rect(first.x, first.y, 0.0, first.h),
        ));
    }
    for (col, pair) in model.columns.windows(2).enumerate() {
        let (left, right) = (&pair[0], &pair[1]);
        let from = left.x + left.w;
        out.push((
            Edge::Column { col },
            rect(from, left.y, right.x - from, left.h),
        ));
    }
    if let Some(last) = model.columns.last() {
        let col = model.columns.len() - 1;
        out.push((
            Edge::Column { col },
            rect(last.x + last.w, last.y, 0.0, last.h),
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
