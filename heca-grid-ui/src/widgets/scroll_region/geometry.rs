//! Geometry for ScrollRegion.

use super::*;

impl ScrollRegion {
    /// Total content extent along the scroll axis (max child **natural** bottom
    /// relative to this region's top, never less than the viewport height).
    /// Uses `+ applied_offset` to recover natural positions from the shifted
    /// bounds.
    pub(super) fn content_extent(&self) -> f64 {
        let vp = self.base.bounds;
        let off = self.applied_offset;
        let mut max_bottom = vp.loc.y + vp.size.h;
        for c in &self.base.children {
            let b = c.base().bounds;
            // natural bottom = shifted bottom + applied shift.
            let bottom = b.loc.y + b.size.h + off;
            if bottom > max_bottom {
                max_bottom = bottom;
            }
        }
        (max_bottom - vp.loc.y).max(vp.size.h)
    }
    /// The **raw** content extent along each axis — what the children actually occupy, with no
    /// floor at the viewport. [`content_extent`](Self::content_extent) clamps to the viewport
    /// because that is what the scroll maths wants; centring needs the unclamped truth, since the
    /// whole question is *how much smaller than the viewport the content is*.
    pub(super) fn raw_extent(&self) -> (f64, f64) {
        let vp = self.base.bounds;
        let (mut right, mut bottom) = (vp.loc.x, vp.loc.y);
        for c in &self.base.children {
            let b = c.base().bounds;
            right = right.max(b.loc.x + b.size.w + self.applied_offset_x);
            bottom = bottom.max(b.loc.y + b.size.h + self.applied_offset);
        }
        (right - vp.loc.x, bottom - vp.loc.y)
    }
    /// How far to push the content down and right so it sits in the **middle of a region it does
    /// not fill** — `(0, 0)` unless [`reveal_align`](Self::reveal_align) is
    /// [`Center`](RevealAlign::Center) and there is leftover room on that axis.
    ///
    /// This is the other half of what "centred" means, and the half that has nothing to do with
    /// scrolling. `center_on` can only ever put the target in the middle by *moving the content
    /// past it*; when the content is smaller than the viewport there is nothing to move — the
    /// offset clamps to `0` and everything lands hard against the top-left corner. A map of one
    /// workspace, or a strip of three columns on a wide screen, is exactly that case, and pinning
    /// it to the corner is what an overview must never look like.
    ///
    /// **It centres the cursor, not the box.** When something in the subtree asks to be revealed,
    /// the pad puts *that* in the middle, so focusing the leftmost of three columns brings it to
    /// the centre and pushes its neighbours off the right edge — the same picture scrolling gives
    /// when the content is larger. Only with nothing to follow does it centre the content as a
    /// whole, which is the honest answer for a surface with no cursor in it.
    ///
    /// Deliberately **not** `justify_content: center` on the region: flexbox centring overflows
    /// equally in both directions once the content is larger, which puts the first row above the
    /// scrollable area and makes it permanently unreachable. This applies only while there is
    /// slack, so it can never do that — CSS's `safe center`, which taffy has no spelling for.
    pub(super) fn center_pad(&self) -> (f64, f64) {
        if self.reveal_align != RevealAlign::Center {
            return (0.0, 0.0);
        }
        let vp = self.base.bounds;
        let (content_w, content_h) = self.raw_extent();
        // Natural (pre-shift) rect of whatever asked to be seen — or, while nothing is asking, the
        // last thing that did. See `last_reveal_rect`: a grid stops asking while the pointer drives
        // its cursor, and the pad must hold rather than re-centre on the whole strip.
        let target = crate::component::reveal_target_in(&self.base.children)
            .map(|r| {
                Rectangle::new(
                    Point::new(
                        r.loc.x + self.applied_offset_x,
                        r.loc.y + self.applied_offset,
                    ),
                    r.size,
                )
            })
            .or(self.last_reveal_rect);
        let over = self.overscroll;
        let pad = |on: bool, vp_start: f64, vp_len: f64, content: f64, tgt: Option<(f64, f64)>| {
            if !on || (content > vp_len && !over) {
                // Overflowing an axis with ends: the scroll offset does the centring, and it
                // clamps — so the first and last of anything rest against the edge instead of
                // floating in half a screen of nothing.
                return 0.0;
            }
            // **What is centred depends on whether there is anything you cannot see.**
            //
            // When the content **fits**, centre the *content*: everything is on screen, so pulling
            // one card to the middle can only push its neighbours off the edges — which is exactly
            // what an exposé must never do. The map showed a screen of empty space on the left with
            // the last column clipped on the right, because the cursor card was being centred in a
            // strip that already fitted twice over.
            //
            // When it **overflows** — only reachable here with `overscroll`, which has no ends to
            // clamp against — centre the cursor, because then there really is something off-screen
            // and following it is the whole point.
            match tgt.filter(|_| content > vp_len) {
                Some((start, len)) => vp_len / 2.0 - (start - vp_start) - len / 2.0,
                None => (vp_len - content) / 2.0,
            }
        };
        // Temporary instrumentation: the exposé's content measured 727 in a 730
        // viewport — fitting — while this pushed it 185px down, which is the "it overflows, follow
        // the cursor" branch. Six analytic diagnoses in a row have been wrong; this prints what the
        // decision is actually made on.
        #[cfg(debug_assertions)]
        if std::env::var_os("HECA_TRACE_PAD").is_some() {
            eprintln!(
                "[heca] center_pad: vp={:.0}x{:.0} content={content_w:.0}x{content_h:.0} \
                 over={over} target={:?} align={:?}",
                vp.size.w,
                vp.size.h,
                target.map(|r| (r.loc.y, r.size.h)),
                self.reveal_align,
            );
        }
        (
            pad(
                self.axes.is_horizontal(),
                vp.loc.x,
                vp.size.w,
                content_w,
                target.map(|r| (r.loc.x, r.size.w)),
            ),
            pad(
                self.axes.is_vertical(),
                vp.loc.y,
                vp.size.h,
                content_h,
                target.map(|r| (r.loc.y, r.size.h)),
            ),
        )
    }
    /// The range the vertical offset may take: `[0, max]` normally.
    ///
    /// **With [`overscroll`](Self::overscroll) it gains a viewport of room at each end.** Without
    /// that the wheel is dead on an overscrolling surface: the position is carried by the centring
    /// pad, the content usually fits, so `max_offset` is `0`, the range is a single point and every
    /// notch is a no-op. A viewport either side is enough to look past anything and bounded enough
    /// that the view cannot be flicked off into empty space.
    pub(super) fn offset_bounds_y(&self) -> (f64, f64) {
        let max = self.max_offset();
        // **Overscroll is a cushion past the ends, and content that fits has no ends.** Applied
        // unconditionally it handed a full viewport of travel in each direction to a map with
        // nothing off screen, so the wheel carried the whole picture out of the window and there
        // was no way back but the keyboard.
        match self.overscroll && self.axes.is_vertical() && max > 0.0 {
            true => (-self.base.bounds.size.h, max + self.base.bounds.size.h),
            false => (0.0, max),
        }
    }
    /// The horizontal counterpart of [`offset_bounds_y`](Self::offset_bounds_y).
    pub(super) fn offset_bounds_x(&self) -> (f64, f64) {
        let max = self.max_offset_x();
        match self.overscroll && self.axes.is_horizontal() && max > 0.0 {
            true => (-self.base.bounds.size.w, max + self.base.bounds.size.w),
            false => (0.0, max),
        }
    }
    /// Largest valid vertical offset (≥ 0). Always 0 when the vertical axis is
    /// disabled. When a horizontal bar shows it reserves that bar's gutter, so the
    /// user can scroll the last row fully **above** the bar instead of leaving it
    /// half-hidden in the gutter — the effective viewport is `viewport_h − gutter`.
    pub(super) fn max_offset(&self) -> f64 {
        if !self.axes.is_vertical() {
            return 0.0;
        }
        let reserve = if self.scrollbars && self.h_overflow() {
            SCROLLBAR_GUTTER
        } else {
            0.0
        };
        (self.content_extent() + reserve - self.base.bounds.size.h).max(0.0)
    }
    /// Total content extent along the **horizontal** axis (max child natural right
    /// relative to this region's left, never less than the viewport width). Uses
    /// `+ applied_offset_x` to recover natural positions from the shifted bounds.
    pub(super) fn content_extent_x(&self) -> f64 {
        let vp = self.base.bounds;
        let off = self.applied_offset_x;
        let mut max_right = vp.loc.x + vp.size.w;
        for c in &self.base.children {
            let b = c.base().bounds;
            let right = b.loc.x + b.size.w + off;
            if right > max_right {
                max_right = right;
            }
        }
        (max_right - vp.loc.x).max(vp.size.w)
    }
    /// Largest valid horizontal offset (≥ 0). Always 0 when the horizontal axis is
    /// disabled. Reserves the vertical bar's gutter (when it shows) so the last
    /// column can clear the bar.
    pub(super) fn max_offset_x(&self) -> f64 {
        if !self.axes.is_horizontal() {
            return 0.0;
        }
        let reserve = if self.scrollbars && self.v_overflow() {
            SCROLLBAR_GUTTER
        } else {
            0.0
        };
        (self.content_extent_x() + reserve - self.base.bounds.size.w).max(0.0)
    }
    /// Whether the vertical axis currently overflows (→ a vertical scrollbar shows).
    /// Independent of the horizontal bar, so each bar can size the other's track
    /// (leaving the corner empty) without recursing through `thumb_rect`.
    pub(super) fn v_overflow(&self) -> bool {
        self.axes.is_vertical() && self.content_extent() > self.base.bounds.size.h + 0.5
    }
    /// Whether the horizontal axis currently overflows (→ a horizontal scrollbar shows).
    pub(super) fn h_overflow(&self) -> bool {
        self.axes.is_horizontal() && self.content_extent_x() > self.base.bounds.size.w + 0.5
    }
    /// The vertical scrollbar thumb rect (in viewport space), or `None` when the
    /// content fits. The track stops short of the horizontal bar's gutter when both
    /// show, so the two never overlap in the bottom-right corner.
    pub(super) fn thumb_rect(&self) -> Option<Rectangle> {
        // `scrollbars(false)` means no bar: no thumb drawn, none to grab, and no gutter reserved.
        // Gating only the gutter left the bar painted — visible, draggable, and taking no space.
        if !self.scrollbars || !self.v_overflow() {
            return None;
        }
        let vp = self.base.bounds;
        let content_h = self.content_extent();
        let track_h = vp.size.h
            - if self.h_overflow() {
                SCROLLBAR_GUTTER
            } else {
                0.0
            };
        let thumb_h = ((vp.size.h / content_h) * track_h)
            .max(MIN_THUMB)
            .min(track_h);
        let max_off = self.max_offset();
        let frac = if max_off > 0.0 {
            (self.scroll_offset.get_untracked() as f64 / max_off).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_y = vp.loc.y + (track_h - thumb_h) * frac;
        let thumb_x = vp.loc.x + vp.size.w - SCROLLBAR_W - SCROLLBAR_EDGE_INSET;
        Some(Rectangle::new(
            Point::new(thumb_x, thumb_y),
            Size::new(SCROLLBAR_W, thumb_h),
        ))
    }
    /// The thumb's **grab lane** — the visible 8px thumb centered inside a wider
    /// `THUMB_HIT_W` click/hover target at the right edge (at the thumb's y), so a
    /// thin thumb is still easy to grab. `None` when not scrollable. Used for
    /// press/hover hit-testing; [`thumb_rect`](Self::thumb_rect) is the painted
    /// (thin) thumb.
    pub(super) fn thumb_hit_rect(&self) -> Option<Rectangle> {
        let t = self.thumb_rect()?;
        let lane_x = self.base.bounds.loc.x + self.base.bounds.size.w - THUMB_HIT_W;
        Some(Rectangle::new(
            Point::new(lane_x, t.loc.y),
            Size::new(THUMB_HIT_W, t.size.h),
        ))
    }
    /// The **horizontal** scrollbar thumb rect (bottom edge, viewport space), or
    /// `None` when the content fits horizontally. Mirrors [`thumb_rect`](Self::thumb_rect).
    pub(super) fn h_thumb_rect(&self) -> Option<Rectangle> {
        if !self.scrollbars || !self.h_overflow() {
            return None;
        }
        let vp = self.base.bounds;
        let content_w = self.content_extent_x();
        let track_w = vp.size.w
            - if self.v_overflow() {
                SCROLLBAR_GUTTER
            } else {
                0.0
            };
        let thumb_w = ((vp.size.w / content_w) * track_w)
            .max(MIN_THUMB)
            .min(track_w);
        let max_off = self.max_offset_x();
        let frac = if max_off > 0.0 {
            (self.scroll_offset_x.get_untracked() as f64 / max_off).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_x = vp.loc.x + (track_w - thumb_w) * frac;
        let thumb_y = vp.loc.y + vp.size.h - SCROLLBAR_W - SCROLLBAR_EDGE_INSET;
        Some(Rectangle::new(
            Point::new(thumb_x, thumb_y),
            Size::new(thumb_w, SCROLLBAR_W),
        ))
    }
    /// The horizontal thumb's **grab lane** (a wider `THUMB_HIT_W`-tall target at
    /// the bottom edge). `None` when not scrollable horizontally.
    pub(super) fn h_thumb_hit_rect(&self) -> Option<Rectangle> {
        let t = self.h_thumb_rect()?;
        let lane_y = self.base.bounds.loc.y + self.base.bounds.size.h - THUMB_HIT_W;
        Some(Rectangle::new(
            Point::new(t.loc.x, lane_y),
            Size::new(t.size.w, THUMB_HIT_W),
        ))
    }
    /// Is `pos` over a scrollbar lane — thumb or track — of an axis that actually scrolls?
    ///
    /// One definition, used by hover here and by the paging press, so the two can never disagree
    /// about where the scrollbar is.
    pub(super) fn in_scrollbar_lane(&self, pos: Point, vp: Rectangle) -> bool {
        if !vp.contains(pos) {
            return false;
        }
        let vertical = self.thumb_rect().is_some() && pos.x >= vp.loc.x + vp.size.w - THUMB_HIT_W;
        let horizontal =
            self.h_thumb_rect().is_some() && pos.y >= vp.loc.y + vp.size.h - THUMB_HIT_W;
        vertical || horizontal
    }
    /// A press, before the children: end any stale gesture, then try the scrollbar lanes.
    pub(super) fn press_capture(&mut self, pos: Point, vp: Rectangle, cause: &Event) -> Handled {
        {
            let pos = &pos;
            // A fresh press means any previous gesture is over — so a grab can never survive
            // one, even if this region never saw the release that should have ended it.
            //
            // It should always see that release: `PointerReleased` is part of the pointer set
            // a host owes any tree it mounts, and `release_grabs` below does not care where
            // the release landed. But a host that forwards a subset produces a thumb welded to
            // the cursor with no way back, and that has now happened in three different
            // surfaces. The widget stops being the thing that pays for it: the worst a missing
            // release can do here is survive until the next click.
            self.release_grabs();
            // Grab the thumb via its wider hit lane (the thin visible thumb is
            // easy to miss); `thumb_grab` stores the grab point relative to the
            // *visible* thumb top so the cursor stays pinned to it.
            if let Some(hit) = self.thumb_hit_rect()
                && hit.contains(*pos)
            {
                let t = self.thumb_rect().expect("scrollable: thumb exists");
                self.thumb_grab = Some(pos.y - t.loc.y);
                self.thumb_hovered = true;
                self.base.mark_needs_paint();
                return Handled::Yes;
            }
            if let Some(hit) = self.h_thumb_hit_rect()
                && hit.contains(*pos)
            {
                let t = self.h_thumb_rect().expect("scrollable-x: thumb exists");
                self.h_thumb_grab = Some(pos.x - t.loc.x);
                self.h_thumb_hovered = true;
                self.base.mark_needs_paint();
                return Handled::Yes;
            }
            // Click in the scrollbar TRACK but off the thumb → page toward the
            // click (a screenful in that direction), the standard scrollbar
            // affordance — and ARM the press-and-hold repeat: holding the press
            // keeps paging (see `tick`) until release. The thumb-grab checks
            // above already returned for a hit on the thumb itself, so reaching
            // here means the empty track.
            if self.thumb_rect().is_some() {
                let lane_x = vp.loc.x + vp.size.w - THUMB_HIT_W;
                if pos.x >= lane_x && vp.contains(*pos) {
                    self.page_toward(false, *pos, Some(cause));
                    self.track_repeat = Some(TrackRepeat {
                        horizontal: false,
                        pos: *pos,
                        next_in: TRACK_REPEAT_DELAY,
                    });
                    return Handled::Yes;
                }
            }
            if self.h_thumb_rect().is_some() {
                let lane_y = vp.loc.y + vp.size.h - THUMB_HIT_W;
                if pos.y >= lane_y && vp.contains(*pos) {
                    self.page_toward(true, *pos, Some(cause));
                    self.track_repeat = Some(TrackRepeat {
                        horizontal: true,
                        pos: *pos,
                        next_in: TRACK_REPEAT_DELAY,
                    });
                    return Handled::Yes;
                }
            }
            // Not a scrollbar gesture: let it through. The children are walked by `dispatch`,
            // and `clips_children` already stops a press outside the viewport reaching content
            // that has been scrolled out of sight.
            Handled::No
        }
    }
    /// A move, before the children: hover state and the scrollbar lanes, then any drag in flight.
    pub(super) fn move_capture(&mut self, pos: Point, vp: Rectangle, cause: &Event) -> Handled {
        {
            let pos = &pos;
            // Track thumb-lane hover for the highlight affordance. A drag in
            // progress keeps handling moves even after the cursor leaves.
            let lane_hit = self.thumb_hit_rect().is_some_and(|h| h.contains(*pos));
            if lane_hit != self.thumb_hovered {
                self.thumb_hovered = lane_hit;
                self.base.mark_needs_paint();
            }
            let h_lane_hit = self.h_thumb_hit_rect().is_some_and(|h| h.contains(*pos));
            if h_lane_hit != self.h_thumb_hovered {
                self.h_thumb_hovered = h_lane_hit;
                self.base.mark_needs_paint();
            }
            if let Some(grab) = self.thumb_grab {
                let content_h = self.content_extent();
                let max_off = (content_h - vp.size.h).max(0.0);
                let track_h = vp.size.h;
                let thumb_h = ((vp.size.h / content_h) * track_h).max(MIN_THUMB);
                let thumb_top = pos.y - grab;
                let frac = if track_h - thumb_h > 0.0 {
                    ((thumb_top - vp.loc.y) / (track_h - thumb_h)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                self.set_offset_y((frac * max_off) as f32, Some(cause));
                Handled::Yes
            } else if let Some(grab) = self.h_thumb_grab {
                let content_w = self.content_extent_x();
                let max_off = (content_w - vp.size.w).max(0.0);
                let track_w = vp.size.w;
                let thumb_w = ((vp.size.w / content_w) * track_w).max(MIN_THUMB);
                let thumb_left = pos.x - grab;
                let frac = if track_w - thumb_w > 0.0 {
                    ((thumb_left - vp.loc.x) / (track_w - thumb_w)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                self.set_offset_x((frac * max_off) as f32, Some(cause));
                Handled::Yes
            } else if let Some(tr) = &mut self.track_repeat {
                // A held track press is a grab: track the cursor (the repeat
                // pages toward wherever it is now) and consume the move.
                tr.pos = *pos;
                Handled::Yes
            } else if self.in_scrollbar_lane(*pos, vp) {
                // The cursor is over a scrollbar lane, so the lane owns it: without this the
                // move falls through and the row *underneath* the scrollbar lights up as
                // hovered, which reads as pointing at content the user is not pointing at.
                // The whole lane, not just the thumb — the track is part of the control (a
                // press there pages), so it is not content either.
                Handled::Yes
            } else {
                // No drag in flight: the children get it (walked by `dispatch`, and skipped
                // entirely when the cursor is outside this region — see `clips_children`).
                Handled::No
            }
        }
    }
}
