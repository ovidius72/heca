//! How tall each pane of a column is, and how a drag changes that.

use super::{Column, Pane};
use crate::layout::types::*;

/// Minimum height (logical px) a pane may be shrunk to by a manual resize, so a
/// pane never collapses to a thin sliver.
pub const MIN_PANE_HEIGHT: f64 = 100.0;

impl Column {
    /// **Make room for a pane that is about to exist.**
    ///
    /// A height a pane was dragged to is a *preference*. Two panes dragged to fill the column
    /// between them hold the whole of it, and a third arriving has nothing left: it was assigned a
    /// single pixel, and the pass that scales the column to fit shaved barely one per cent off the
    /// other two. The pane was there, in the column, and could not be seen — which is what
    /// "splitting a third time pushes the last pane off the screen" actually was. Nothing was pushed anywhere, so checking that the heights summed to the
    /// column found nothing: they always did.
    ///
    /// ⚠️ **This is the ADD, not the distribution.** It would be simpler to reserve a floor for
    /// every pane inside `pane_heights`, and it is wrong there: that runs on every drag too,
    /// and a boundary must move space between its own two panes and *nothing else* — held by
    /// `a_resize_leaves_every_other_pane_where_it_was`. Making room is something a new pane does,
    /// once, at the moment it arrives.
    ///
    /// The preferences are scaled rather than dropped, so the proportions the user dragged survive
    /// as far as the column still allows.
    pub fn make_room_for_one_more(&mut self, working_height: f64, gaps: f64) {
        let after = self.panes.len() + 1;
        let room_before = self.room(working_height, gaps);
        let available = working_height - gaps * (after as f64 + 1.0);
        let floor = MIN_PANE_HEIGHT.min(available / after as f64).max(1.0);
        let pixels = |pane: &Pane| pane.height_share.map(|share| share * room_before);
        let pinned: f64 = self.panes.iter().filter_map(pixels).sum();
        // What the pinned panes may hold and still leave every other pane its floor.
        let unpinned = self.panes.iter().filter(|p| p.height_share.is_none()).count();
        let ceiling = available - floor * (unpinned + 1) as f64;
        let scale = match pinned <= ceiling || pinned <= 0.0 {
            true => 1.0,
            false => (ceiling / pinned).max(0.0),
        };
        // The heights the panes were dragged to stay what they are (scaled when they cannot all
        // fit), re-expressed against the room the column has once the new pane is in it.
        for pane in &mut self.panes {
            if let Some(px) = pixels(pane) {
                let px = if scale < 1.0 { (px * scale).max(floor) } else { px };
                pane.height_share = Some(px / available);
            }
        }
    }

    /// The room a column's panes share: its height less the gaps around and between them.
    fn room(&self, working_height: f64, gaps: f64) -> f64 {
        working_height - gaps * (self.panes.len() as f64 + 1.0)
    }

    /// Give pane `idx` a height of `height` logical px, as a share of the room this column has in
    /// `working_height` — so it keeps its proportion when the window changes size. The one place
    /// pixels become a share; nothing else is told how tall a pane is. Nothing happens when the
    /// column has no room to measure against.
    pub fn set_pane_height(&mut self, idx: usize, height: f64, working_height: f64, gaps: f64) {
        let room = self.room(working_height, gaps);
        if room <= 0.0 {
            return;
        }
        if let Some(pane) = self.panes.get_mut(idx) {
            pane.height_share = Some((height / room).clamp(f64::EPSILON, 1.0));
        }
    }

    /// **How tall each pane is** in a column given `working_height` of room — worked out from the
    /// panes' height shares each time it is asked, so it is never out of date, and the shared
    /// content holds no window's pixels.
    ///
    /// This is NIRI's height distribution algorithm simplified.
    pub fn pane_heights(&self, working_height: f64, gaps: f64) -> Vec<f64> {
        let pane_count = self.panes.len();
        if pane_count == 0 {
            return Vec::new();
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
            if let Some(share) = pane.height_share {
                let fixed_h = share * available_height;
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
            // certain point started moving the top one too.
            let per_auto = (height_left / auto_count as f64).max(0.0);
            let auto_floor = min_h.min(per_auto).max(1.0);
            let auto_height = per_auto.max(auto_floor);
            for (i, pane) in self.panes.iter().enumerate() {
                if pane.height_share.is_none() {
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
        // panes have taken theirs. With one pane resized (so carrying a `height_share`) and two
        // sharing the rest, that floor could exceed what was left, the heights summed to more than
        // the column, and the last pane was pushed off the bottom of the screen.
        //
        // Which is why it only happened in the column that had been resized, and why the effective
        // minimum differed between that column and a freshly created one. Scaling both ways makes "the panes exactly fill the column" true by
        // construction rather than in one direction only; a column too small for every pane's floor
        // degrades proportionally, which is what the floor's own note already asks for.
        let total_pane_height: f64 = sizes.iter().map(|s| s.h).sum();
        if total_pane_height > 0.0 && available_height > 0.0 {
            let scale = available_height / total_pane_height;
            for size in &mut sizes {
                size.h *= scale;
            }
        }

        sizes.into_iter().map(|s| s.h).collect()
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
    /// Same key, same "grow me", opposite edge: on the bottom pane `j`/`k` seem to work, on the
    /// middle and top panes they seem reversed. This is a consequence of [`resize_pane_height`](Self::resize_pane_height)'s
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
    /// This used to pin **one** pane and let [`pane_heights`](Self::pane_heights)
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
    /// which reads as having disappeared.
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
        let room = self.room(working_height, gaps);
        if room <= 0.0 {
            return;
        }
        let heights = self.pane_heights(working_height, gaps);
        // The pane's **actual current** height in this window, so a pane that was still
        // auto-sized (an even split) does not jump on the first drag delta.
        let height_of = |idx: usize| heights.get(idx).copied().unwrap_or(0.0);
        let mine = height_of(pane_idx);
        let theirs = height_of(other);
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
        self.panes[pane_idx].height_share = Some((mine + delta) / room);
        self.panes[other].height_share = Some((theirs - delta) / room);
    }
}
