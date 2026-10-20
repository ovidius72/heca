use super::types::*;

mod heights;
mod pane;
#[cfg(test)]
mod pane_height_tests;

pub use heights::MIN_PANE_HEIGHT;
pub use pane::Pane;

/// A column of panes arranged according to a layout mode.
///
/// In NIRI terms, this is a `Column<W>` that contains `Vec<Tile<W>>`.
/// In heca, we generalize it to support multiple layout modes within the column.
#[derive(Debug, Clone)]
pub struct Column {
    pub id: ColumnId,
    /// Optional user-visible name for this column.
    pub name: Option<String>,
    /// Panes in this column. Must be non-empty.
    pub panes: Vec<Pane>,
    /// Desired width of this column.
    pub width: ColumnWidth,
    /// Previous width saved while this column is zoomed to the viewport.
    pub zoom_restore_width: Option<ColumnWidth>,
    /// Whether this column is full-width.
    pub is_full_width: bool,
    /// Whether this column is pending fullscreen.
    pub is_pending_fullscreen: bool,
    /// Whether this column is pending maximized.
    pub is_pending_maximized: bool,
}

impl Column {
    pub fn new(id: ColumnId, first_pane: Pane, width: ColumnWidth) -> Self {
        Self {
            id,
            name: None,
            panes: vec![first_pane],
            width,
            zoom_restore_width: None,
            is_full_width: false,
            is_pending_fullscreen: false,
            is_pending_maximized: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    pub fn sizing_mode(&self) -> SizingMode {
        if self.is_pending_fullscreen {
            SizingMode::Fullscreen
        } else if self.is_pending_maximized {
            SizingMode::Maximized
        } else {
            SizingMode::Normal
        }
    }

    pub fn is_zoomed(&self) -> bool {
        self.zoom_restore_width.is_some()
    }

    /// **How wide this column is on screen**, and never narrower than
    /// [`MIN_COLUMN_WIDTH`](crate::layout::scrolling::MIN_COLUMN_WIDTH).
    ///
    /// The floor is here because this is where the width is *decided*. It used to live only in the
    /// resize handlers — which stop **you** dragging a column to nothing, and stopped nothing else:
    /// a proportion of a squeezed working area resolved straight through zero, and every pane in the
    /// column laid out with no width at all. A column narrower than its floor
    /// overflows the viewport instead, which is what a scrolling column layout is for.
    ///
    /// One constant, not two: the zoomed branch floored at a private `50.0` while everything else
    /// used 150.
    pub fn resolve_width(&self, working_width: f64, gaps: f64) -> f64 {
        let floor = crate::layout::scrolling::MIN_COLUMN_WIDTH;
        if self.is_zoomed() || self.is_full_width {
            return (working_width - gaps * 2.0).max(floor);
        }
        match self.width {
            ColumnWidth::Proportion(p) => ((working_width - gaps) * p - gaps).max(floor),
            ColumnWidth::Fixed(w) => w.max(floor),
        }
    }

    /// Add a pane at a specific position.
    pub fn add_pane_at(&mut self, idx: usize, pane: Pane) {
        self.panes.insert(idx, pane);
    }

    /// Remove a pane by index. Which pane each window then has focused is the window's own, kept by
    /// id in its view — there is no shared active index here to renumber.
    pub fn remove_pane(&mut self, idx: usize) -> Option<Pane> {
        (idx < self.panes.len()).then(|| self.panes.remove(idx))
    }
}
