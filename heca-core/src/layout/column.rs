use super::animation::{Animated, Animation, AnimationConfig};
use super::types::*;
use crate::runtime::{PaneClosePolicy, PaneRuntime};

/// Minimum height (logical px) a pane may be shrunk to by a manual resize, so a
/// pane never collapses to a thin sliver.
pub const MIN_PANE_HEIGHT: f64 = 100.0;

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
    /// Cached computed width (updated after resize).
    pub computed_width: f64,
    /// Cached pane sizes.
    pub pane_sizes: Vec<Size>,
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

    pub fn is_zoomed(&self) -> bool {
        self.zoom_restore_width.is_some()
    }

    pub fn resolve_width(&self, working_width: f64, gaps: f64) -> f64 {
        if self.is_zoomed() || self.is_full_width {
            return (working_width - gaps * 2.0).max(50.0);
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

        // Per-pane floor: MIN_PANE_HEIGHT, but degrade gracefully when the column
        // genuinely can't fit every pane at the min (tiny window / many panes) —
        // never let a pane collapse to a ~1px sliver and disappear (#4).
        let min_h = MIN_PANE_HEIGHT.min(available_height / pane_count as f64).max(1.0);

        // First pass: assign fixed heights, count auto panes.
        for (i, pane) in self.panes.iter().enumerate() {
            if let Some(fixed_h) = pane.preferred_height {
                let h = fixed_h.clamp(min_h, height_left.max(min_h));
                sizes[i].h = h;
                height_left -= h;
                auto_count -= 1;
            }
        }

        // Second pass: distribute remaining height to auto panes.
        if auto_count > 0 {
            let auto_height = (height_left / auto_count as f64).max(min_h);
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
        self.resize_pane_height(self.active_pane_idx, delta, working_height, gaps);
    }

    /// Grow pane `pane_idx` by `delta` logical px, **taking the space from the pane on the other
    /// side of the boundary being dragged** (the one below it, or the one above when it is last).
    /// No-op for single-pane columns, an out-of-range index, or when the neighbour has no room left.
    /// Used by the keyboard resize (active pane), the mouse divider drag (any pane), and RPC.
    ///
    /// # A boundary moves space between its OWN two panes
    ///
    /// This used to pin **one** pane and let [`compute_pane_sizes`](Self::compute_pane_sizes)
    /// redistribute the remainder over every pane that was still auto-sized. With two panes the
    /// only auto pane *was* the neighbour, so it looked right. With three it was plainly wrong:
    /// dragging the top boundary took space from the bottom pane as well, which had nothing to do
    /// with it —
    ///
    /// ```text
    /// start                                 289 / 289 / 289
    /// after dragging the TOP boundary down  668 / 100 / 100   ← the third pane was never touched
    /// ```
    ///
    /// — and the third collapsed to [`MIN_PANE_HEIGHT`], jammed against the bottom of the column,
    /// which reads as having disappeared (Antonio, driving, 2026-08-11; F004/P084/T413).
    ///
    /// So the transfer is explicit and local: **both** sides of the boundary are pinned, by equal
    /// and opposite amounts, and every other pane keeps exactly the height it had — there is no
    /// remainder left for anything else to absorb. A pane the user has never dragged stays auto, so
    /// an untouched column still splits evenly and still reflows when the window changes.
    pub fn resize_pane_height(
        &mut self,
        pane_idx: usize,
        delta: f64,
        working_height: f64,
        gaps: f64,
    ) {
        if self.panes.len() <= 1 || pane_idx >= self.panes.len() {
            return;
        }
        // The pane on the other side of the boundary. A divider is named by the pane **above** it
        // (`mouse::resize::divider_at`), so that is normally the one below; the last pane has no
        // boundary beneath it, and the keyboard can aim at it, so it trades with the one above
        // instead. Either way "grow me" grows *me*.
        let other = if pane_idx + 1 < self.panes.len() {
            pane_idx + 1
        } else {
            pane_idx - 1
        };
        let height_of = |col: &Self, idx: usize| {
            // The pane's **actual current** height, not a fixed 200px default: a pane that was
            // still auto-sized (an even split) would jump to ~200px on the first drag delta
            // otherwise. Falls back to 200px only when no layout has been computed yet (a pure
            // unit test).
            col.panes[idx].preferred_height.unwrap_or_else(|| {
                col.pane_sizes
                    .get(idx)
                    .map(|s| s.h)
                    .filter(|h| *h > 0.0)
                    .unwrap_or(200.0)
            })
        };
        let mine = height_of(self, pane_idx);
        let theirs = height_of(self, other);
        // How far the boundary may travel: I cannot shrink past the floor, and neither can they.
        // When both are already at it there is no room at all — stop rather than reaching past the
        // neighbour for space, which is the whole point of this function.
        let (lo, hi) = (MIN_PANE_HEIGHT - mine, theirs - MIN_PANE_HEIGHT);
        if hi < lo {
            return;
        }
        let delta = delta.clamp(lo, hi);
        if delta == 0.0 {
            return;
        }
        self.panes[pane_idx].preferred_height = Some(mine + delta);
        self.panes[other].preferred_height = Some(theirs - delta);
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
    /// User-set display name (from rename). `Some` **overrides** the process-derived
    /// title everywhere it's shown; `None` means the name tracks the running process.
    pub custom_name: Option<String>,
    pub runtime: PaneRuntime,
    pub close_policy: PaneClosePolicy,
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
            custom_name: None,
            runtime: PaneRuntime::default(),
            close_policy: PaneClosePolicy::default(),
            preferred_height: None,
            move_offset: Animated::Static(Point::default()),
            interactive_move_offset: Point::default(),
        }
    }
}
