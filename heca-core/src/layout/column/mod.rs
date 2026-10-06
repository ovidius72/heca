use super::animation::{Animated, Animation, AnimationConfig};
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
    /// Currently active pane index.
    pub active_pane_idx: usize,
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
    /// Animation offset during column moves (e.g., when a column is added/removed nearby).
    pub move_offset: Animated<f64>,
}

impl Column {
    pub fn new(id: ColumnId, first_pane: Pane, width: ColumnWidth) -> Self {
        Self {
            id,
            name: None,
            panes: vec![first_pane],
            active_pane_idx: 0,
            width,
            zoom_restore_width: None,
            is_full_width: false,
            is_pending_fullscreen: false,
            is_pending_maximized: false,
            move_offset: Animated::Static(0.0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    pub fn active_pane(&self) -> Option<&Pane> {
        self.panes.get(self.active_pane_idx)
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        self.panes.get_mut(self.active_pane_idx)
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
    /// column laid out with no width at all (F003/P082/T478). A column narrower than its floor
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

    /// Activate a pane by index.
    pub fn activate_pane(&mut self, idx: usize) -> bool {
        if idx >= self.panes.len() || self.active_pane_idx == idx {
            return false;
        }
        self.active_pane_idx = idx;
        true
    }

    /// Focus up (previous pane in column).
    pub fn focus_up(&mut self) -> bool {
        self.activate_pane(self.active_pane_idx.saturating_sub(1))
    }

    /// Focus down (next pane in column).
    pub fn focus_down(&mut self) -> bool {
        self.activate_pane((self.active_pane_idx + 1).min(self.panes.len().saturating_sub(1)))
    }

    /// Move the active pane up within the column.
    pub fn move_up(&mut self) -> bool {
        if self.active_pane_idx == 0 {
            return false;
        }
        self.panes
            .swap(self.active_pane_idx, self.active_pane_idx - 1);
        self.active_pane_idx -= 1;
        true
    }

    /// Move the active pane down within the column.
    pub fn move_down(&mut self) -> bool {
        if self.active_pane_idx + 1 >= self.panes.len() {
            return false;
        }
        self.panes
            .swap(self.active_pane_idx, self.active_pane_idx + 1);
        self.active_pane_idx += 1;
        true
    }

    /// Add a pane at a specific position.
    pub fn add_pane_at(&mut self, idx: usize, pane: Pane) {
        self.panes.insert(idx, pane);
        if idx <= self.active_pane_idx {
            self.active_pane_idx += 1;
        }
    }

    /// Remove a pane by index.
    pub fn remove_pane(&mut self, idx: usize) -> Option<Pane> {
        if idx >= self.panes.len() {
            return None;
        }
        let pane = self.panes.remove(idx);
        if self.active_pane_idx >= self.panes.len() && !self.panes.is_empty() {
            self.active_pane_idx = self.panes.len() - 1;
        } else if idx < self.active_pane_idx {
            self.active_pane_idx -= 1;
        }
        Some(pane)
    }

    /// Get the render offset for this column (includes move animation).
    pub fn render_offset(&self) -> f64 {
        self.move_offset.current()
    }

    /// Animate this column moving from an offset.
    pub fn animate_move_from(&mut self, from_x: f64, config: AnimationConfig) {
        let current = self.move_offset.current();
        let anim = Animation::new(from_x + current, 0.0, config);
        self.move_offset = Animated::Animating {
            animation: anim,
            from: from_x + current,
            to: 0.0,
        };
    }
}
