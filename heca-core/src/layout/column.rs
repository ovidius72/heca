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
        self.pane_sizes.clear(); // Invalidate cache
    }

    /// **Make room for a pane that is about to exist.**
    ///
    /// A height a pane was dragged to is a *preference*. Two panes dragged to fill the column
    /// between them hold the whole of it, and a third arriving has nothing left: it was assigned a
    /// single pixel, and the pass that scales the column to fit shaved barely one per cent off the
    /// other two. The pane was there, in the column, and could not be seen — which is what
    /// "splitting a third time pushes the last pane off the screen" actually was (Antonio, driving,
    /// 2026-09-03/04). Nothing was pushed anywhere, so checking that the heights summed to the
    /// column found nothing: they always did.
    ///
    /// ⚠️ **This is the ADD, not the distribution.** It would be simpler to reserve a floor for
    /// every pane inside `compute_pane_sizes`, and it is wrong there: that runs on every drag too,
    /// and a boundary must move space between its own two panes and *nothing else* — held by
    /// `a_resize_leaves_every_other_pane_where_it_was`. Making room is something a new pane does,
    /// once, at the moment it arrives.
    ///
    /// The preferences are scaled rather than dropped, so the proportions the user dragged survive
    /// as far as the column still allows.
    pub fn make_room_for_one_more(&mut self, working_height: f64, gaps: f64) {
        let after = self.panes.len() + 1;
        let available = working_height - gaps * (after as f64 + 1.0);
        let floor = MIN_PANE_HEIGHT.min(available / after as f64).max(1.0);
        let pinned: f64 = self.panes.iter().filter_map(|p| p.preferred_height).sum();
        // What the pinned panes may hold and still leave every other pane its floor.
        let unpinned = self
            .panes
            .iter()
            .filter(|p| p.preferred_height.is_none())
            .count();
        let ceiling = available - floor * (unpinned + 1) as f64;
        if pinned <= ceiling || pinned <= 0.0 {
            return;
        }
        let scale = (ceiling / pinned).max(0.0);
        for pane in &mut self.panes {
            if let Some(h) = pane.preferred_height {
                pane.preferred_height = Some((h * scale).max(floor));
            }
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
        let min_h = MIN_PANE_HEIGHT
            .min(available_height / pane_count as f64)
            .max(1.0);

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
            // **The floor is bounded by the room actually left to them.** `min_h` above is worked
            // out against the whole column; the auto panes are sharing only what the *fixed* ones
            // did not take. Forcing the column-wide floor here is what made the heights sum past
            // the column — and once they did, the proportional scale below shrank every pane to
            // compensate, including the two the user had just pinned by dragging their boundary.
            // Which is the symptom: resizing the middle pane moved the bottom edge, and then at a
            // certain point started moving the top one too (Antonio, driving, 2026-09-03).
            let per_auto = (height_left / auto_count as f64).max(0.0);
            let auto_floor = min_h.min(per_auto).max(1.0);
            let auto_height = per_auto.max(auto_floor);
            for (i, pane) in self.panes.iter().enumerate() {
                if pane.preferred_height.is_none() {
                    sizes[i].h = auto_height;
                }
            }
        }

        // Third pass: **the panes fill the column exactly** — scaled proportionally, in whichever
        // direction is needed.
        //
        // It used to scale only *up*, to fill leftover space, and that left the overflowing case
        // unhandled: the per-pane floor above is worked out from `available / pane_count`, but the
        // auto panes are then floored at it against `height_left` — the room left after the *fixed*
        // panes have taken theirs. With one pane resized (so carrying a `preferred_height`) and two
        // sharing the rest, that floor could exceed what was left, the heights summed to more than
        // the column, and the last pane was pushed off the bottom of the screen.
        //
        // Which is why it only happened in the column that had been resized, and why the effective
        // minimum differed between that column and a freshly created one (Antonio, driving,
        // 2026-09-03). Scaling both ways makes "the panes exactly fill the column" true by
        // construction rather than in one direction only; a column too small for every pane's floor
        // degrades proportionally, which is what the floor's own note already asks for.
        let total_pane_height: f64 = sizes.iter().map(|s| s.h).sum();
        if total_pane_height > 0.0 && available_height > 0.0 {
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

    /// Resize the active pane's height by a delta (pixels): **positive grows it**, whichever edge
    /// has to move to do that. This is the *size* verb — `pane_height_increase` /
    /// `pane_height_decrease` — and it is deliberately not the one the resize mode uses; see
    /// [`move_active_pane_boundary`](Self::move_active_pane_boundary).
    pub fn resize_active_pane_height(&mut self, delta: f64, working_height: f64, gaps: f64) {
        self.resize_pane_height(self.active_pane_idx, delta, working_height, gaps);
    }

    /// Move the **boundary the active pane owns** by `delta` logical px **along the axis**, so
    /// positive is *down the screen*. The direction is the whole point: see
    /// [`move_pane_boundary`](Self::move_pane_boundary).
    ///
    /// A pane has two edges, and this is the one *below* it — except for the last pane, which has
    /// none and trades with the one above instead. To aim at the other edge deliberately, use
    /// [`move_active_pane_top_boundary`](Self::move_active_pane_top_boundary).
    pub fn move_active_pane_boundary(&mut self, delta: f64, working_height: f64, gaps: f64) {
        self.move_pane_boundary(self.active_pane_idx, delta, working_height, gaps);
    }

    /// Move the boundary **above** the active pane by `delta` logical px, positive being *down the
    /// screen* like every other resize verb — so a positive delta here **shrinks** the active pane
    /// from the top, and a negative one grows it upwards.
    ///
    /// The counterpart to [`move_active_pane_boundary`](Self::move_active_pane_boundary), which
    /// takes the edge below. Which of a pane's two edges moves is the column's own business: a
    /// caller says *which edge*, never which pane index the divider happens to be named by.
    ///
    /// **No-op for the first pane**, which has nothing above it to trade with.
    pub fn move_active_pane_top_boundary(&mut self, delta: f64, working_height: f64, gaps: f64) {
        let Some(above) = self.active_pane_idx.checked_sub(1) else {
            return;
        };
        self.move_pane_boundary(above, delta, working_height, gaps);
    }

    /// Move the boundary pane `pane_idx` owns — the one **below** it, or the one **above** when it
    /// is the last pane — by `delta` logical px **along the axis**: positive is **down**.
    ///
    /// # Why this exists beside "grow me"
    ///
    /// `j`/`k` are **directional**: the user expects an edge to travel one way. "Grow the active
    /// pane" only agrees with that while the edge that moves is on the same side of the pane —
    ///
    /// - a pane with a boundary **below** it grows by moving its **bottom** edge down;
    /// - the **last** pane has no boundary beneath it, so it grows by moving its **top** edge up.
    ///
    /// Same key, same "grow me", opposite edge. Antonio, driving, 2026-08-11: *"prefix+r work fine
    /// at the third one at the bottom. The center and the one at the top j and k act the
    /// opposite."* This is a consequence of [`resize_pane_height`](Self::resize_pane_height)'s
    /// local transfer (F004/P084/T413), not a regression it introduced: before it, a resize spread
    /// the change over every auto-sized pane, so no single divider visibly moved and the ambiguity
    /// did not read.
    ///
    /// So the keyboard names a **boundary and a direction**, which is what the mouse already did —
    /// a divider is named by the pane above it and a drag down moves it down. One convention, one
    /// call: for the last pane, moving the only boundary it has (the one above) *down* means the
    /// pane **shrinks**, which is the correct and consistent reading — the divider went the way the
    /// key says.
    pub fn move_pane_boundary(
        &mut self,
        pane_idx: usize,
        delta: f64,
        working_height: f64,
        gaps: f64,
    ) {
        if self.panes.len() <= 1 || pane_idx >= self.panes.len() {
            return;
        }
        // Name the divider by the pane **above** it — `mouse::resize::divider_at`'s convention —
        // so "grow that pane" and "move this boundary down" are the same statement.
        let above = pane_idx.min(self.panes.len() - 2);
        self.resize_pane_height(above, delta, working_height, gaps);
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

#[cfg(test)]
mod pane_height_tests {
    use super::*;

    fn column_of(n: usize) -> Column {
        let mut col = Column::new(
            ColumnId(1),
            Pane::new(PaneId(1), "p1"),
            ColumnWidth::Proportion(1.0),
        );
        for i in 1..n {
            col.add_pane_at(i, Pane::new(PaneId(i as u64 + 1), format!("p{}", i + 1)));
        }
        col
    }

    /// **The panes exactly fill the column, always.**
    ///
    /// A pane that has been resized carries a fixed height, and the floor the others were given was
    /// worked out against the *whole* column rather than the room those fixed panes had left — so
    /// three panes in a resized column summed to more than the column and the last one was pushed
    /// off the bottom of the screen. It happened only in a column that had been resized, which is
    /// exactly why a freshly created one looked fine (Antonio, driving, 2026-09-03).
    #[test]
    fn panes_never_sum_to_more_than_the_column() {
        let working = 800.0;
        let gaps = 4.0;
        for fixed in [None, Some(600.0), Some(700.0), Some(60.0)] {
            let mut col = column_of(3);
            if let Some(h) = fixed {
                col.panes[0].preferred_height = Some(h);
            }
            col.compute_pane_sizes(working, gaps);
            let total: f64 = col.pane_sizes.iter().map(|s| s.h).sum();
            let available = working - gaps * (col.panes.len() as f64 + 1.0);
            assert!(
                total <= available + 0.5,
                "three panes summed to {total} in {available} of room (first pane fixed at {fixed:?})"
            );
        }
    }

    /// …and they fill it, rather than leaving a gap at the bottom.
    #[test]
    fn panes_fill_the_column_they_are_given() {
        let working = 800.0;
        let gaps = 4.0;
        let mut col = column_of(3);
        col.panes[0].preferred_height = Some(200.0);
        col.compute_pane_sizes(working, gaps);
        let total: f64 = col.pane_sizes.iter().map(|s| s.h).sum();
        let available = working - gaps * 4.0;
        assert!((total - available).abs() < 0.5, "{total} of {available}");
    }

    /// **A boundary moves space between its own two panes and nothing else** — right up to the
    /// limit.
    ///
    /// Dragging the middle pane's lower boundary moved the bottom edge, and then at a certain point
    /// started moving the *top* one too: the untouched panes were being forced above the room left
    /// to them, the heights summed past the column, and the proportional scale that keeps the column
    /// full then shrank every pane — including the two the drag had just pinned (Antonio, driving,
    /// 2026-09-03).
    /// **A column is never narrower than its floor, whatever the window did** (F003/P082/T478).
    ///
    /// `MIN_COLUMN_WIDTH` used to be enforced only by the resize handlers, which stop *you* dragging
    /// a column to nothing and stopped nothing else. Two sidebars in a narrow window squeezed the
    /// working area to zero, a proportion of zero resolved to zero, and every pane in the column
    /// laid out with no width at all — still lettered by `prefix+/`, its keycap drawn beside a pane
    /// with no inside.
    #[test]
    fn a_column_keeps_its_floor_when_the_working_area_collapses() {
        let floor = crate::layout::scrolling::MIN_COLUMN_WIDTH;
        let col = column_of(1);
        assert!(
            col.resolve_width(0.0, 8.0) >= floor,
            "a column in a working area squeezed to nothing still has a width (got {})",
            col.resolve_width(0.0, 8.0),
        );
        assert!(
            col.resolve_width(150.0, 8.0) >= floor,
            "and so does one whose proportion resolves below the floor (got {})",
            col.resolve_width(150.0, 8.0),
        );
    }

    /// A column with room resolves to what its proportion actually asks for — the floor is a floor,
    /// not a width.
    #[test]
    fn a_column_with_room_is_sized_by_its_proportion_not_the_floor() {
        let col = column_of(1);
        let w = col.resolve_width(1200.0, 8.0);
        assert!(
            w > crate::layout::scrolling::MIN_COLUMN_WIDTH,
            "a wide working area gives a wide column (got {w})",
        );
    }

    #[test]
    fn a_resize_leaves_every_other_pane_where_it_was() {
        let working = 400.0;
        let gaps = 4.0;
        let mut col = column_of(3);
        // Two panes pinned by earlier drags, leaving the third less than its floor. That is the
        // case: the third was then forced up to the column-wide floor, the heights summed past the
        // column, and the scale that keeps the column full pulled the two pinned panes off the
        // sizes the user had just set.
        col.panes[0].preferred_height = Some(250.0);
        col.panes[1].preferred_height = Some(MIN_PANE_HEIGHT);
        col.compute_pane_sizes(working, gaps);

        assert!(
            (col.pane_sizes[0].h - 250.0).abs() < 0.5,
            "a pinned pane keeps the height it was given (got {})",
            col.pane_sizes[0].h
        );
        assert!(
            (col.pane_sizes[1].h - MIN_PANE_HEIGHT).abs() < 0.5,
            "and so does the one on the other side of that boundary (got {})",
            col.pane_sizes[1].h
        );
    }

    /// **A column too small for every pane's floor degrades proportionally** rather than pushing
    /// the last pane out — which is what the floor's own note asks for.
    #[test]
    fn a_short_column_shrinks_every_pane_rather_than_losing_one() {
        let mut col = column_of(5);
        col.compute_pane_sizes(200.0, 2.0);
        let available = 200.0 - 2.0 * 6.0;
        let total: f64 = col.pane_sizes.iter().map(|s| s.h).sum();
        assert!(total <= available + 0.5, "{total} of {available}");
        assert!(
            col.pane_sizes.iter().all(|s| s.h > 0.0),
            "no pane collapses to nothing"
        );
    }
}
