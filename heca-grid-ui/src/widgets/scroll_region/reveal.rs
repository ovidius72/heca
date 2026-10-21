//! Reveal for ScrollRegion.

use super::*;

impl ScrollRegion {
    /// Scroll minimally so the given rect — read from a descendant's current
    /// `bounds` (visual/on-screen space) — is fully inside the viewport. If the
    /// rect is already visible, nothing happens; if it sits above the viewport,
    /// the view scrolls to put its top at the viewport top; if below, its
    /// bottom at the viewport bottom. This is the **scroll-into-view** a host
    /// container (e.g. the sidebar) uses to keep the keyboard cursor in view when
    /// the selection moves: pass the selected descendant's `base().bounds`.
    ///
    /// The rect is in *visual* space (what you read from a component's bounds at
    /// the current scroll position); the widget recovers the natural position
    /// internally via its baked shift (`applied_offset`), so the host never has
    /// to track the scroll offset or do offset math itself. Minimal movement —
    /// it won't jump if the item is already on screen.
    pub fn ensure_visible(&mut self, visual_rect: Rectangle) {
        self.sync_shift();
        let vp = self.base.bounds;
        let off = self.applied_offset;
        let natural_top = visual_rect.loc.y + off;
        let natural_bot = natural_top + visual_rect.size.h;
        let cur = self.scroll_offset.get_untracked() as f64;
        let vp_top = vp.loc.y;
        let vp_bot = vp.loc.y + vp.size.h;
        if cur > natural_top - vp_top {
            // Item's top is above the viewport — scroll up to align tops.
            self.scroll_to((natural_top - vp_top) as f32);
        } else if cur < natural_bot - vp_bot {
            // Item's bottom is below the viewport — scroll down to align bottoms.
            self.scroll_to((natural_bot - vp_bot) as f32);
        }
        // else already fully visible — no scroll.
    }
    /// Scroll so the given rect — read from a descendant's current `bounds`, exactly as
    /// [`ensure_visible`](Self::ensure_visible) takes it — sits at the **centre** of the viewport.
    ///
    /// Unconditional, unlike `ensure_visible`: a target already on screen but off-centre is brought
    /// back to the middle, because "keep it centred" is the whole behaviour. The clamp inside
    /// [`scroll_to`](Self::scroll_to) does the rest, so at the ends of the content the target simply
    /// stops short of the centre rather than the view scrolling into empty space. A caller that
    /// wants it centred at the ends too gives the content slack of its own — half a viewport of
    /// padding at each end is the usual way, and it is the caller's because only the caller knows
    /// whether that empty space belongs in its surface.
    /// **Both axes**, unlike [`ensure_visible`](Self::ensure_visible), which is vertical only: a
    /// horizontal region centres horizontally, and a two-axis one does both. A strip of cards is
    /// the case that needs it — "the one you are on sits in the middle" is the same sentence
    /// whichever way the cards are laid out.
    pub fn center_on(&mut self, visual_rect: Rectangle) {
        self.sync_shift();
        let vp = self.base.bounds;
        if self.axes.is_vertical() {
            let natural_top = visual_rect.loc.y + self.applied_offset;
            let mid = natural_top + visual_rect.size.h / 2.0;
            self.scroll_to((mid - vp.loc.y - vp.size.h / 2.0) as f32);
        }
        if self.axes.is_horizontal() {
            let natural_left = visual_rect.loc.x + self.applied_offset_x;
            let mid = natural_left + visual_rect.size.w / 2.0;
            self.scroll_to_x((mid - vp.loc.x - vp.size.w / 2.0) as f32);
        }
    }
    /// Convenience: scroll so the direct child at `index` is fully visible. Use
    /// this for a list whose selectable units are direct children (e.g. a flat
    /// list of `Item`s). For a nested selectable unit (a sidebar row inside a
    /// `DockFrame`/group), use [`ensure_visible`](Self::ensure_visible) with the
    /// descendant's `bounds` instead. Out-of-range index is a no-op.
    pub fn scroll_to_child(&mut self, index: usize) {
        if let Some(child) = self.base.children.get(index) {
            let rect = child.base().bounds;
            self.ensure_visible(rect);
        }
    }
    /// Reserve layout space for each visible scrollbar, so content is laid out **beside** the bar
    /// rather than under it.
    ///
    /// Clipping alone is not enough, and the difference is visible: a card laid out full width and
    /// then trimmed at the lane loses its rounded corner and ends on a hard vertical cut, because
    /// it still *is* wider than the space it has. Reserving the gutter as padding on that one side
    /// makes the card the width it appears to be.
    ///
    /// Applied after layout, so it takes effect on the next pass — which is how a classic
    /// scrollbar behaves everywhere. It cannot oscillate: narrowing content can only make it taller,
    /// never shorter, so a region that overflows vertically still overflows once it has reserved
    /// the space.
    pub(super) fn reserve_scrollbar_gutters(&mut self) {
        // Read the overflow flags before borrowing the layout mutably.
        let (v_overflow, h_overflow) = (self.v_overflow(), self.h_overflow());
        // A step is a fraction of the font, and this compares against a fixed gutter — so the
        // authored padding has to become pixels before the two can be weighed against each other.
        let font = self.base.font;
        let layout = &mut self.base.style.layout;
        // What the other side of each axis already insets by. The bar takes the LARGER of that and
        // the gutter, never the sum: where the padding is already roomy enough the bar simply sits
        // in it, both sides stay equal, and nothing looks lopsided. Space is only added when the
        // bar genuinely needs more than is already there.
        let base_x = layout.padding_x.unwrap_or(layout.padding).resolve(font);
        let base_y = layout.padding_y.unwrap_or(layout.padding).resolve(font);
        let gutter = SCROLLBAR_GUTTER as f32;
        // The reservation is a measured number, not an authored step — it is what the bar needs.
        let want_x = v_overflow.then(|| crate::style::Space::Px(base_x.max(gutter)));
        let want_y = h_overflow.then(|| crate::style::Space::Px(base_y.max(gutter)));
        if layout.padding_right != want_x || layout.padding_bottom != want_y {
            layout.padding_right = want_x;
            layout.padding_bottom = want_y;
            self.base.mark_needs_paint();
        }
    }
    /// Keep the descendant that asked to be visible in view — the keyboard's half of scrolling.
    ///
    /// Any widget reporting [`Component::wants_visible`] (focus by default; a list's navigation
    /// cursor by override) is scrolled to, in **every** region it is ever placed inside. That is
    /// the point of it living here: "make the view follow the cursor" is not something each host
    /// wires for each list, forgets in the next one, and rediscovers as a bug.
    ///
    /// It moves when the target moves or its viewport changes size. The remembered position is
    /// the target's natural one, so scrolling by hand does not change it and the view does not snap back — the
    /// user can always look somewhere else.
    ///
    /// [`reveal_align`](Self::reveal_align) decides *where* it lands: minimally in view, or centred.
    pub(super) fn follow_revealed(&mut self) {
        let target = crate::component::reveal_target_in(&self.base.children);
        let Some(rect) = target else {
            self.last_revealed = None;
            return;
        };
        // Natural = what layout produced, before this region's shift was baked in.
        let natural = (
            rect.loc.x + self.applied_offset_x,
            rect.loc.y + self.applied_offset,
        );
        self.last_reveal_rect = Some(Rectangle::new(Point::new(natural.0, natural.1), rect.size));
        let revealed = (natural.0, natural.1, self.base.bounds.size);
        if self.last_revealed == Some(revealed) {
            return;
        }
        self.last_revealed = Some(revealed);
        match self.reveal_align {
            RevealAlign::Minimal => self.ensure_visible(rect),
            // With `overscroll` the centring pad already tracks the target on every sync, and a
            // scroll on top of it would centre the same thing twice and overshoot by that much.
            // What the offset carries there is **where the user has wheeled to**, and moving the
            // cursor is a new instruction that supersedes it — so it goes back to zero and the pad
            // puts the target dead centre, instead of centring it half a screen off for good.
            RevealAlign::Center if self.overscroll => {
                self.scroll_to(0.0);
                self.scroll_to_x(0.0);
            }
            RevealAlign::Center => self.center_on(rect),
        }
    }
}
