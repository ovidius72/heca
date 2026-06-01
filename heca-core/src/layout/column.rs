use super::animation::{Animation, AnimationConfig, Animated};
use super::types::*;

/// A column of panes arranged according to a layout mode.
///
/// In NIRI terms, this is a `Column<W>` that contains `Vec<Tile<W>>`.
/// In heca, we generalize it to support multiple layout modes within the column.
#[derive(Debug, Clone)]
pub struct Column {
    pub id: ColumnId,
    /// Panes in this column. Must be non-empty.
    pub panes: Vec<Pane>,
    /// Currently active pane index.
    pub active_pane_idx: usize,
    /// Desired width of this column.
    pub width: ColumnWidth,
    /// Whether this column is full-width.
    pub is_full_width: bool,
    /// Whether this column is pending fullscreen.
    pub is_pending_fullscreen: bool,
    /// Whether this column is pending maximized.
    pub is_pending_maximized: bool,
    /// Animation offset during column moves (e.g., when a column is added/removed nearby).
    pub move_offset: Animated<f64>,
    /// Cached computed width (updated after resize).
    pub computed_width: f64,
    /// Cached pane sizes.
    pub pane_sizes: Vec<Size>,
}

impl Column {
    pub fn new(id: ColumnId, first_pane: Pane, width: ColumnWidth) -> Self {
        Self {
            id,
            panes: vec![first_pane],
            active_pane_idx: 0,
            width,
            is_full_width: false,
            is_pending_fullscreen: false,
            is_pending_maximized: false,
            move_offset: Animated::Static(0.0),
            computed_width: 0.0,
            pane_sizes: vec![],
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

    pub fn resolve_width(&self, working_width: f64, gaps: f64) -> f64 {
        if self.is_full_width {
            return working_width - gaps * 2.0;
        }
        match self.width {
            ColumnWidth::Proportion(p) => (working_width - gaps) * p - gaps,
            ColumnWidth::Fixed(w) => w,
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
        self.panes.swap(self.active_pane_idx, self.active_pane_idx - 1);
        self.active_pane_idx -= 1;
        true
    }

    /// Move the active pane down within the column.
    pub fn move_down(&mut self) -> bool {
        if self.active_pane_idx + 1 >= self.panes.len() {
            return false;
        }
        self.panes.swap(self.active_pane_idx, self.active_pane_idx + 1);
        self.active_pane_idx += 1;
        true
    }

    /// Add a pane at a specific position.
    pub fn add_pane_at(&mut self, idx: usize, pane: Pane) {
        self.panes.insert(idx, pane);
        if idx <= self.active_pane_idx {
            self.active_pane_idx += 1;
        }
        self.pane_sizes.clear(); // Invalidate cache
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
        self.pane_sizes.clear();
        Some(pane)
    }

    /// Compute pane sizes within this column given the available height.
    ///
    /// This is NIRI's height distribution algorithm simplified.
    pub fn compute_pane_sizes(&mut self, working_height: f64, gaps: f64) {
        let pane_count = self.panes.len();
        if pane_count == 0 {
            return;
        }

        let total_gaps = gaps * (pane_count as f64 + 1.0);
        let available_height = working_height - total_gaps;

        let mut sizes = vec![Size::default(); pane_count];
        let mut height_left = available_height;
        let mut auto_count = pane_count;

        // First pass: assign fixed heights, count auto panes.
        for (i, pane) in self.panes.iter().enumerate() {
            if let Some(fixed_h) = pane.preferred_height {
                let h = fixed_h.min(height_left.max(1.0));
                sizes[i].h = h;
                height_left -= h;
                auto_count -= 1;
            }
        }

        // Second pass: distribute remaining height to auto panes.
        if auto_count > 0 {
            let auto_height = (height_left / auto_count as f64).max(1.0);
            for (i, pane) in self.panes.iter().enumerate() {
                if pane.preferred_height.is_none() {
                    sizes[i].h = auto_height;
                }
            }
        }

        // Third pass: if all panes have fixed heights and there's leftover space,
        // scale them up proportionally so the column is always full.
        let total_pane_height: f64 = sizes.iter().map(|s| s.h).sum();
        if total_pane_height < available_height && total_pane_height > 0.0 {
            let scale = available_height / total_pane_height;
            for size in &mut sizes {
                size.h *= scale;
            }
        }

        // Set widths to column width.
        let width = self.computed_width;
        for size in &mut sizes {
            size.w = width;
        }

        self.pane_sizes = sizes;
    }

    /// Get the render offset for this column (includes move animation).
    pub fn render_offset(&self) -> f64 {
        self.move_offset.current()
    }

    /// Resize the active pane's height by a delta (pixels).
    /// Only affects panes with preferred_height; others remain auto.
    pub fn resize_active_pane_height(&mut self, delta: f64, working_height: f64, gaps: f64) {
        if self.panes.len() <= 1 {
            return; // No resize when only one pane
        }
        let idx = self.active_pane_idx;
        let current = self.panes[idx].preferred_height.unwrap_or(200.0);
        let new_h = (current + delta).clamp(50.0, working_height - gaps * 2.0);
        self.panes[idx].preferred_height = Some(new_h);
        self.compute_pane_sizes(working_height, gaps);
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

impl Pane {
    /// Animate this pane moving vertically from an offset.
    pub fn animate_move_y_from(&mut self, from_y: f64, config: AnimationConfig) {
        self.move_offset.to_static();
        self.move_offset = Animated::Animating {
            animation: Animation::new(0.0, 1.0, config),
            from: Point::new(0.0, from_y),
            to: Point::new(0.0, 0.0),
        };
    }

    /// Animate this pane moving from a 2D offset.
    pub fn animate_move_from(&mut self, from: Point, config: AnimationConfig) {
        self.move_offset.to_static();
        self.move_offset = Animated::Animating {
            animation: Animation::new(0.0, 1.0, config),
            from,
            to: Point::new(0.0, 0.0),
        };
    }
}

/// A pane within a column.
///
/// This is heca's equivalent of NIRI's `Tile<W>` — it wraps the actual content
/// (terminal, neovim, browser) and tracks its layout state.
#[derive(Debug, Clone)]
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    /// Preferred fixed height (None = auto).
    pub preferred_height: Option<f64>,
    /// Move animation offset (entry/exit animations).
    pub move_offset: Animated<Point>,
    /// Offset applied during interactive move Starting phase (rubberband).
    /// Cleared on transition to Moving. Not used by entry/exit animations.
    pub interactive_move_offset: Point,
}

impl Pane {
    pub fn new(id: PaneId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            preferred_height: None,
            move_offset: Animated::Static(Point::default()),
            interactive_move_offset: Point::default(),
        }
    }
}
