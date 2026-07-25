//! [`ScrollRegion`] — an embeddable vertical scroll viewport.
//!
//! A scrollable column: children are laid out top-to-bottom at their natural
//! height (the layout engine never flex-shrinks them, so the column overflows),
//! and the visible window is the [`ScrollRegion`]'s own bounds. Content beyond
//! the viewport is clipped (renderer `PushClip`/`PopClip`).
//!
//! **Mechanism — same as the whole-page scroll.** The page scroll shifts the
//! tree's bounds by the scroll delta and lets the framebuffer clip the overflow.
//! This widget reuses that pattern for a sub-region: it bakes `-scroll_offset`
//! into its children's bounds (so paint, hit-testing, and DnD all see the
//! *visual* position — bounds === what's drawn) and clips to its own rect via
//! `PushClip` (a sub-region has no framebuffer, so it needs an explicit clip).
//! Because bounds always match the visual, pointer routing and the drag
//! framework's `source_at`/`resolve_at` (which hit-test against bounds) just
//! work while scrolled — no separate translation layer for DnD.
//!
//! **Layout reset.** Shifting bounds is destructive, so a fresh layout pass
//! (resize / font / content change) would compound the shift. The layout engine
//! calls [`Component::on_layout`] post-order after re-computing bounds; the
//! region resets its `applied_offset` there (children are back at natural), so
//! the next paint re-applies the shift from scratch instead of compounding.
//!
//! Interaction: the wheel (`Event::Scroll`) advances the offset (clamped to
//! `[0, max_offset]`), and the auto-shown scrollbar thumb is draggable. The
//! offset is also exposed as a reactive [`Signal<f32>`] the host can read or
//! drive directly.
//!
//! Scrolls **vertically by default**; opt into horizontal (or both) with
//! [`ScrollRegion::horizontal`] / [`both`](ScrollRegion::both) — plain wheel scrolls
//! vertically, `Shift`+wheel horizontally, and each overflowing axis grows its own
//! thumb (the vertical one on the right edge, the horizontal one on the bottom). Each
//! thumb is a theme-**accent** grip that brightens on hover/drag (mirroring
//! [`MarkerGroup`](crate::widgets::MarkerGroup)'s grip bar) and sits in a wider
//! invisible grab lane so a thin thumb is easy to click. Implements
//! [`StyleExt`](crate::builders::StyleExt), so it can double as a **scrollable
//! surface** (`.background(..).border(..)`). Nested regions compose: the wheel is
//! offered to children first, so the innermost hovered scrollable wins. A distinct
//! scrollbar color token and `PageUp`/`PageDown` (app actions) are future work.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{
    paint_child, route_event, shift_subtree, Base, Component, Event, Handled, PaintCx,
};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::style::{Direction, Length, Spacing};
use heca_core::layout::{Point, Rectangle, Size};

/// Visible scrollbar thumb width (logical px).
const SCROLLBAR_W: f64 = 8.0;
/// Gap between the scrollbar and the content edge.
const SCROLLBAR_PAD: f64 = 2.0;
/// Grab lane width (logical px): the visible thumb is `SCROLLBAR_W`, but the
/// click/hover target is this wide (centered on the right edge) so a thin thumb
/// is still easy to grab — mirrors [`MarkerGroup`](crate::widgets::MarkerGroup)'s
/// `GRIP_W` invisible grab padding around its thin bar. Without it the 8px thumb
/// misses clicks too often.
const THUMB_HIT_W: f64 = 16.0;
/// Minimum thumb height so a very long list still has a grabbable thumb.
const MIN_THUMB: f64 = 24.0;
/// Space reserved along an edge for a visible scrollbar (the lane the thumb sits
/// in): the thumb plus its padding on both sides. Content is clipped short of it,
/// and the *perpendicular* track stops before it, so a bar never overlaps content
/// or the other bar in the corner.
const SCROLLBAR_GUTTER: f64 = SCROLLBAR_W + 2.0 * SCROLLBAR_PAD;
/// Wheel step as a fraction of the viewport height per "line" of delta. The
/// winit wheel delta is already in lines, so one notch (delta ≈ 1) scrolls ~10%
/// of the viewport — gentle in a small sidebar, scales up for a tall one. (The
/// previous build multiplied by a fixed line count × font, which made each notch
/// jump ~75% of a small viewport and overshoot.)
const WHEEL_STEP_FRAC: f64 = 0.1;
/// Rest-glow spread radius (px) for a STYLED region (a scrollable panel) — its
/// share of the theme rest halo; a frameless region has no surface and no glow.
const SURFACE_GLOW_RADIUS: f32 = 12.0;
/// How far the content clip is widened on an axis this region does **not** scroll,
/// so a child's rest-glow halo (drawn outside the child's own bounds) isn't
/// scissored flat against the edge and read as a cut-off row. Sized to the surface
/// glow spread above.
const GLOW_BLEED: f64 = SURFACE_GLOW_RADIUS as f64;
/// Delay (seconds) before a held track-press starts repeating its paging.
const TRACK_REPEAT_DELAY: f32 = 0.35;
/// Interval (seconds) between repeated pages while the track press stays held.
const TRACK_REPEAT_INTERVAL: f32 = 0.1;

/// A held press in a scrollbar **track** (off the thumb): the standard
/// press-and-hold affordance — page once immediately, then keep paging toward
/// the cursor until released (pausing when the thumb reaches it).
struct TrackRepeat {
    /// Which axis's track is held (`true` = the horizontal bar).
    horizontal: bool,
    /// Latest cursor position (updated by `PointerMoved` while held).
    pos: Point,
    /// Seconds until the next repeat fires (counts down in `tick`).
    next_in: f32,
}

/// Which axes a [`ScrollRegion`] scrolls. Default [`Vertical`](ScrollAxes::Vertical)
/// keeps every existing caller unchanged; opt into horizontal with
/// [`horizontal`](ScrollRegion::horizontal) / [`both`](ScrollRegion::both).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScrollAxes {
    /// Vertical only (the historical default).
    #[default]
    Vertical,
    /// Horizontal only.
    Horizontal,
    /// Both axes.
    Both,
}

impl ScrollAxes {
    /// Does this configuration scroll vertically?
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
    /// Does this configuration scroll horizontally?
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }
}

/// An embeddable scroll viewport hosting children that may overflow it.
///
/// Build with [`ScrollRegion::new`], append children via [`Parent::child`], and
/// read/drive the position via [`ScrollRegion::scroll_offset`] /
/// [`ScrollRegion::scroll_to`] (vertical) and
/// [`scroll_offset_x`](ScrollRegion::scroll_offset_x) /
/// [`scroll_to_x`](ScrollRegion::scroll_to_x) (horizontal). A scrollbar appears
/// automatically on each axis whose content overflows the viewport.
///
/// **Axes.** Vertical by default (back-compat). [`horizontal`](Self::horizontal)
/// or [`both`](Self::both) opt into horizontal scrolling; the wheel scrolls the
/// vertical axis, `Shift`+wheel the horizontal one, and a 2-D trackpad delta
/// drives both. Only enabled axes shift/clip/scrollbar.
///
/// **Scrollable surface.** `ScrollRegion` implements [`StyleExt`], so a plain one
/// is frameless while `.background(..).border(..).radius(..)` makes it a framed,
/// scrollable panel — all values from the [`Theme`](crate::Theme), none hardcoded.
///
/// **Wheel gating.** `Event::Scroll` carries no position, so the default
/// broadcast router (`route_event`) can't hit-test it — an inline scroll region
/// would swallow *every* wheel event in the tree. To avoid that, the region
/// tracks whether the cursor is over it via `PointerMoved` and only consumes a
/// scroll when hovered (and scrollable). The wheel is offered to **children
/// first**, so with nested regions the *innermost* hovered scrollable wins (each
/// gates on its own hover) and an outer whole-page region only scrolls when no
/// descendant consumed the event.
pub struct ScrollRegion {
    base: Base,
    /// Which axes scroll (default [`ScrollAxes::Vertical`]).
    axes: ScrollAxes,
    /// Vertical scroll offset (content px shifted up). 0 = top.
    scroll_offset: Signal<f32>,
    /// Horizontal scroll offset (content px shifted left). 0 = left. Inert unless
    /// [`axes`](Self::axes) includes horizontal.
    scroll_offset_x: Signal<f32>,
    /// The vertical shift currently baked into the children's bounds (=
    /// `scroll_offset` at the last [`sync_shift`](Self::sync_shift)). Bounds hold
    /// `natural - applied_offset`; geometry helpers recover natural as `bounds +
    /// applied_offset`. Reset to 0 by [`on_layout`](Component::on_layout) when
    /// layout re-computes bounds to natural.
    applied_offset: f64,
    /// Horizontal counterpart of [`applied_offset`](Self::applied_offset).
    applied_offset_x: f64,
    /// While dragging the vertical thumb: the y-offset (content px) from the
    /// thumb's top where the grab landed, so the grab point stays under the
    /// cursor. `None` when not dragging.
    thumb_grab: Option<f64>,
    /// While dragging the horizontal thumb: the x-offset from the thumb's left.
    h_thumb_grab: Option<f64>,
    /// Whether the cursor is currently over this region. Updated from
    /// `PointerMoved`/`PointerPressed`; gates `Event::Scroll` so an inline
    /// region only swallows the wheel when actually hovered.
    hovered: bool,
    /// Whether the cursor is over the vertical scrollbar thumb's grab lane. Drives
    /// the hover affordance (the thumb brightens, like [`MarkerGroup`](crate::widgets::MarkerGroup)'s
    /// grip bar).
    thumb_hovered: bool,
    /// Whether the cursor is over the horizontal scrollbar thumb's grab lane.
    h_thumb_hovered: bool,
    /// A held track-press currently auto-repeating its paging (see [`TrackRepeat`]);
    /// `None` when no track press is held.
    track_repeat: Option<TrackRepeat>,
}

impl ScrollRegion {
    /// A new vertical scroll region.
    pub fn new() -> Self {
        let mut base = Base::new();
        // Deliberately NOT focusable / not a tab-stop and it binds NO keys: heca is
        // tmux-style, so plain keys belong to the underlying app (terminal/editor).
        // Keyboard scrolling is driven by the HOST through prefix-gated, configurable
        // scroll actions that call `scroll_to`/`scroll_by`/`ensure_visible` — never by
        // the widget swallowing raw keys. See docs/overlay-design.md (scroll actions).
        base.style.layout.direction = Direction::Column;
        // A scroll viewport must be allowed to be SMALLER than its content — that
        // is the whole point of it. Flexbox defaults fight this twice: a flex item's
        // `min-height` is `auto` (= its content size) and this crate sets
        // `flex_shrink: 0` so explicit widget sizes are never squished. Left at the
        // defaults, a region inside a bounded parent (a sized `Dialog` panel) grows
        // to its content and **overflows the panel instead of scrolling** — no
        // overflow, so no scrollbar. Opting out of both here means a region scrolls
        // wherever it is put, without every caller having to know this.
        base.style.layout.min_width = Some(Length::Px(0.0));
        base.style.layout.min_height = Some(Length::Px(0.0));
        base.style.layout.flex_shrink = Some(1.0);
        // Breathing room so the first and last rows don't sit flush against the
        // clip edge, and a real gap between children — rows that touch are hard to
        // scan. Both are theme SPACING TOKENS, not literals, so they scale with the
        // font, the size variant and UI zoom (a px value tuned at one font size is
        // wrong at every other). A caller can still override either.
        base.style.layout.pad_spacing_y = Some(Spacing::Sm);
        base.style.layout.gap_spacing = Some(Spacing::Md);
        Self {
            base,
            axes: ScrollAxes::default(),
            scroll_offset: signal(0.0),
            scroll_offset_x: signal(0.0),
            applied_offset: 0.0,
            applied_offset_x: 0.0,
            thumb_grab: None,
            h_thumb_grab: None,
            hovered: false,
            thumb_hovered: false,
            h_thumb_hovered: false,
            track_repeat: None,
        }
    }

    /// Scroll horizontally only (children overflow left↔right).
    pub fn horizontal(mut self) -> Self {
        self.axes = ScrollAxes::Horizontal;
        self
    }

    /// Scroll on both axes.
    pub fn both(mut self) -> Self {
        self.axes = ScrollAxes::Both;
        self
    }

    /// Set the scrolling axes explicitly (default [`ScrollAxes::Vertical`]).
    pub fn axes(mut self, axes: ScrollAxes) -> Self {
        self.axes = axes;
        self
    }

    /// The reactive vertical scroll offset (content px). Read or drive it from the
    /// host: `region.scroll_offset().get_untracked()` / `.set(v)`. Use
    /// [`scroll_to`](Self::scroll_to) to set with clamping.
    pub fn scroll_offset(&self) -> Signal<f32> {
        self.scroll_offset
    }

    /// The reactive **horizontal** scroll offset (content px). Only meaningful when
    /// [`axes`](Self::axes) includes horizontal. Use [`scroll_to_x`](Self::scroll_to_x)
    /// to set with clamping.
    pub fn scroll_offset_x(&self) -> Signal<f32> {
        self.scroll_offset_x
    }

    /// Set the horizontal scroll offset, clamped to `[0, max_offset_x]`, bake it
    /// into the children's bounds, and request a repaint. Returns the clamped value.
    pub fn scroll_to_x(&mut self, offset: f32) -> f32 {
        let max = self.max_offset_x() as f32;
        let v = offset.clamp(0.0, max);
        self.scroll_offset_x.set(v);
        self.sync_shift();
        v
    }

    /// Set the scroll offset, clamped to `[0, max_offset]`, bake it into the
    /// children's bounds immediately, and request a repaint. Returns the clamped
    /// value actually applied. Prefer this over raw `scroll_offset().set()` —
    /// it keeps the shifted bounds (used for paint, hit-testing, and DnD) in
    /// sync with the offset in the same call.
    pub fn scroll_to(&mut self, offset: f32) -> f32 {
        let max = self.max_offset() as f32;
        let v = offset.clamp(0.0, max);
        self.scroll_offset.set(v);
        self.sync_shift();
        v
    }

    /// Scroll by `delta` content px (signed: positive = down), clamped to
    /// `[0, max_offset]`. The wheel and click-track paging go through here; host
    /// scroll actions may call it too.
    fn scroll_by(&mut self, delta: f64) {
        let next = self.scroll_offset.get_untracked() as f64 + delta;
        self.scroll_to(next as f32);
    }

    /// One track-paging step **toward `pos`** on the given axis: a screenful in
    /// the cursor's direction relative to the thumb, and a no-op once the thumb
    /// has reached the cursor (the classic pause-under-the-pointer behavior).
    /// Shared by the initial track press and each held-press repeat.
    fn page_toward(&mut self, horizontal: bool, pos: Point) {
        if horizontal {
            if let Some(thumb) = self.h_thumb_rect() {
                let dir = if pos.x < thumb.loc.x {
                    -1.0
                } else if pos.x > thumb.loc.x + thumb.size.w {
                    1.0
                } else {
                    return; // thumb reached the cursor — pause
                };
                let next = self.scroll_offset_x.get_untracked() as f64
                    + dir * self.base.bounds.size.w;
                self.scroll_to_x(next as f32);
            }
        } else if let Some(thumb) = self.thumb_rect() {
            let dir = if pos.y < thumb.loc.y {
                -1.0
            } else if pos.y > thumb.loc.y + thumb.size.h {
                1.0
            } else {
                return; // thumb reached the cursor — pause
            };
            self.scroll_by(dir * self.base.bounds.size.h);
        }
    }

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

    /// Total content extent along the scroll axis (max child **natural** bottom
    /// relative to this region's top, never less than the viewport height).
    /// Uses `+ applied_offset` to recover natural positions from the shifted
    /// bounds.
    fn content_extent(&self) -> f64 {
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

    /// Largest valid vertical offset (≥ 0). Always 0 when the vertical axis is
    /// disabled. When a horizontal bar shows it reserves that bar's gutter, so the
    /// user can scroll the last row fully **above** the bar instead of leaving it
    /// half-hidden in the gutter — the effective viewport is `viewport_h − gutter`.
    fn max_offset(&self) -> f64 {
        if !self.axes.is_vertical() {
            return 0.0;
        }
        let reserve = if self.h_overflow() { SCROLLBAR_GUTTER } else { 0.0 };
        (self.content_extent() + reserve - self.base.bounds.size.h).max(0.0)
    }

    /// Total content extent along the **horizontal** axis (max child natural right
    /// relative to this region's left, never less than the viewport width). Uses
    /// `+ applied_offset_x` to recover natural positions from the shifted bounds.
    fn content_extent_x(&self) -> f64 {
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
    fn max_offset_x(&self) -> f64 {
        if !self.axes.is_horizontal() {
            return 0.0;
        }
        let reserve = if self.v_overflow() { SCROLLBAR_GUTTER } else { 0.0 };
        (self.content_extent_x() + reserve - self.base.bounds.size.w).max(0.0)
    }

    /// Whether the vertical axis currently overflows (→ a vertical scrollbar shows).
    /// Independent of the horizontal bar, so each bar can size the other's track
    /// (leaving the corner empty) without recursing through `thumb_rect`.
    fn v_overflow(&self) -> bool {
        self.axes.is_vertical() && self.content_extent() > self.base.bounds.size.h + 0.5
    }

    /// Whether the horizontal axis currently overflows (→ a horizontal scrollbar shows).
    fn h_overflow(&self) -> bool {
        self.axes.is_horizontal() && self.content_extent_x() > self.base.bounds.size.w + 0.5
    }

    /// The vertical scrollbar thumb rect (in viewport space), or `None` when the
    /// content fits. The track stops short of the horizontal bar's gutter when both
    /// show, so the two never overlap in the bottom-right corner.
    fn thumb_rect(&self) -> Option<Rectangle> {
        if !self.v_overflow() {
            return None;
        }
        let vp = self.base.bounds;
        let content_h = self.content_extent();
        let track_h = vp.size.h - if self.h_overflow() { SCROLLBAR_GUTTER } else { 0.0 };
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
        let thumb_x = vp.loc.x + vp.size.w - SCROLLBAR_W - SCROLLBAR_PAD;
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
    fn thumb_hit_rect(&self) -> Option<Rectangle> {
        let t = self.thumb_rect()?;
        let lane_x = self.base.bounds.loc.x + self.base.bounds.size.w - THUMB_HIT_W;
        Some(Rectangle::new(
            Point::new(lane_x, t.loc.y),
            Size::new(THUMB_HIT_W, t.size.h),
        ))
    }

    /// The **horizontal** scrollbar thumb rect (bottom edge, viewport space), or
    /// `None` when the content fits horizontally. Mirrors [`thumb_rect`](Self::thumb_rect).
    fn h_thumb_rect(&self) -> Option<Rectangle> {
        if !self.h_overflow() {
            return None;
        }
        let vp = self.base.bounds;
        let content_w = self.content_extent_x();
        let track_w = vp.size.w - if self.v_overflow() { SCROLLBAR_GUTTER } else { 0.0 };
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
        let thumb_y = vp.loc.y + vp.size.h - SCROLLBAR_W - SCROLLBAR_PAD;
        Some(Rectangle::new(
            Point::new(thumb_x, thumb_y),
            Size::new(thumb_w, SCROLLBAR_W),
        ))
    }

    /// The horizontal thumb's **grab lane** (a wider `THUMB_HIT_W`-tall target at
    /// the bottom edge). `None` when not scrollable horizontally.
    fn h_thumb_hit_rect(&self) -> Option<Rectangle> {
        let t = self.h_thumb_rect()?;
        let lane_y = self.base.bounds.loc.y + self.base.bounds.size.h - THUMB_HIT_W;
        Some(Rectangle::new(
            Point::new(t.loc.x, lane_y),
            Size::new(t.size.w, THUMB_HIT_W),
        ))
    }

    /// Bake the current `scroll_offset` into the children's bounds. Shifts each
    /// direct child's subtree by `applied_offset − scroll_offset` so the bounds
    /// end at `natural − scroll_offset` (the visual position). Idempotent when
    /// already in sync. Called from `paint` and `event` so bounds are always
    /// current for drawing, hit-testing, and DnD.
    fn sync_shift(&mut self) {
        let target_y = self.scroll_offset.get_untracked() as f64;
        let target_x = self.scroll_offset_x.get_untracked() as f64;
        let dy = self.applied_offset - target_y;
        let dx = self.applied_offset_x - target_x;
        if dx != 0.0 || dy != 0.0 {
            for child in self.base.children.iter_mut() {
                shift_subtree(child.as_mut(), dx, dy);
            }
            self.applied_offset = target_y;
            self.applied_offset_x = target_x;
            self.base.mark_needs_paint();
        }
    }
}

impl Default for ScrollRegion {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ScrollRegion {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Layout just re-computed every bound to its natural position — clear the
    /// baked shift so the next `sync_shift` re-applies it from scratch instead
    /// of compounding. (Post-order: children already assigned.)
    ///
    /// The offset is also **re-clamped** here: a relayout can GROW the viewport
    /// (window resize, zoom-out), leaving the stored offset beyond the new max —
    /// re-applying it raw would shift the content past the edge with no
    /// scrollbar left to bring it back. `scroll_to`/`scroll_to_x` clamp against
    /// the freshly-assigned natural bounds and re-bake the shift.
    fn on_layout(&mut self) {
        self.applied_offset = 0.0;
        self.applied_offset_x = 0.0;
        let y = self.scroll_offset.get_untracked();
        let x = self.scroll_offset_x.get_untracked();
        self.scroll_to(y);
        self.scroll_to_x(x);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let vp = self.base.bounds;
        // Styled-surface decoration (background/border/glow/radius from the theme
        // via `StyleExt`) painted in viewport space, before the clipped content — a
        // plain region sets none of these and stays frameless (and, having no
        // surface, carries no glow). A STYLED region (a scrollable panel) without an
        // explicit `.glow(..)` falls back to the theme rest glow, like every surface.
        let s = &self.base.style;
        if (s.visual.fill.is_some() || s.visual.border.is_some()) && s.visual.glow.is_none() {
            cx.rect(
                vp,
                s.visual.fill.unwrap_or(crate::color::Color::TRANSPARENT),
                s.visual.border,
                s.visual.radius,
                cx.rest_glow(SURFACE_GLOW_RADIUS),
            );
        } else {
            cx.paint_base(&self.base);
        }
        // Reserve a gutter for each visible scrollbar so content is never drawn
        // *under* the thumb: clip the content short of the lane on the right (when
        // the vertical bar shows) and/or the bottom (horizontal bar). The thumbs are
        // then painted below in the full viewport rect, sitting in the clear gutter.
        let mut content_clip = vp;
        if self.v_overflow() {
            content_clip.size.w = (content_clip.size.w - SCROLLBAR_GUTTER).max(0.0);
        }
        if self.h_overflow() {
            content_clip.size.h = (content_clip.size.h - SCROLLBAR_GUTTER).max(0.0);
        }
        // Widen the clip on an axis the region does NOT scroll, so a child's GLOW
        // (which paints outside its own bounds — every bordered surface carries a
        // rest halo) isn't scissored flat against the edge, which reads as the row
        // being "cut off". Only the scrolling axis needs a tight clip — that is the
        // one where content genuinely moves through the viewport and must be cut at
        // the boundary. A non-scrolling axis has nothing to hide, so the few px of
        // bleed is free.
        if !self.axes.is_horizontal() {
            content_clip.loc.x -= GLOW_BLEED;
            content_clip.size.w += 2.0 * GLOW_BLEED;
        }
        if !self.axes.is_vertical() {
            content_clip.loc.y -= GLOW_BLEED;
            content_clip.size.h += 2.0 * GLOW_BLEED;
        }
        // Keep the baked shift current (paint takes `&self`, so sync via the
        // signal value; the shift was already applied by the last `event`/layout
        // reset — `applied_offset` matches `scroll_offset` here in steady state).
        cx.with_clip(content_clip, |cx| {
            for child in &self.base.children {
                paint_child(child.as_ref(), cx);
            }
        });
        // Scrollbar thumb on top, in viewport space (not scrolled with content).
        // Theme-driven hover affordance (mirrors MarkerGroup's grip bar): the
        // thumb is a dim accent at rest, brightens when its grab lane is hovered,
        // and is full-bright while dragged — reading as "grab here". Radius and
        // color both come from the theme (no hardcoded radius/Color).
        if let Some(t) = self.thumb_rect() {
            let alpha = if self.thumb_grab.is_some() || self.thumb_hovered {
                cx.theme().colors.interaction.thumb_hover
            } else {
                cx.theme().colors.interaction.thumb_rest
            };
            let theme = cx.theme();
            let color = theme.colors.accent.with_alpha(alpha);
            cx.rect(t, color, None, theme.colors.control_radius(), None);
        }
        // Horizontal scrollbar thumb (bottom edge), same theme-driven affordance.
        if let Some(t) = self.h_thumb_rect() {
            let alpha = if self.h_thumb_grab.is_some() || self.h_thumb_hovered {
                cx.theme().colors.interaction.thumb_hover
            } else {
                cx.theme().colors.interaction.thumb_rest
            };
            let theme = cx.theme();
            let color = theme.colors.accent.with_alpha(alpha);
            cx.rect(t, color, None, theme.colors.control_radius(), None);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        // Bake the current scroll offset into bounds first, so hit-testing and
        // DnD see the visual position (bounds === what's drawn).
        self.sync_shift();
        let vp = self.base.bounds;

        match ev {
            Event::Scroll { delta_x, delta_y } => {
                // Children FIRST — the innermost scrollable under the cursor wins. A
                // nested `ScrollRegion` gates on its own `hovered`, so when a region
                // hosts other scrollables (e.g. a whole-page region wrapping embedded
                // demo regions) the inner one consumes the wheel and this (outer) one
                // stays put; a child that can't scroll the delta's axis returns
                // `No` and the event falls back here.
                if route_event(&mut self.base.children, ev) == Handled::Yes {
                    return Handled::Yes;
                }
                // Only swallow the wheel when the cursor is over this region AND the
                // delta's axis is scrollable. `Event::Scroll` has no position, so the
                // router can't hit-test it; `hovered` (from `PointerMoved`) is our gate.
                // The host already mapped modifiers to axes (plain wheel → `delta_y`,
                // `Shift`+wheel → `delta_x`), so we just consume each axis we can scroll.
                // Anything we don't consume propagates to the host.
                let mut handled = false;
                if self.hovered && *delta_y != 0.0 && self.max_offset() > 0.0 {
                    let next = self.scroll_offset.get_untracked() as f64
                        + (*delta_y as f64) * WHEEL_STEP_FRAC * vp.size.h;
                    self.scroll_to(next as f32);
                    handled = true;
                }
                if self.hovered && *delta_x != 0.0 && self.max_offset_x() > 0.0 {
                    let next = self.scroll_offset_x.get_untracked() as f64
                        + (*delta_x as f64) * WHEEL_STEP_FRAC * vp.size.w;
                    self.scroll_to_x(next as f32);
                    handled = true;
                }
                if handled { Handled::Yes } else { Handled::No }
            }
            Event::PointerPressed { pos } => {
                self.hovered = vp.contains(*pos);
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
                        self.page_toward(false, *pos);
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
                        self.page_toward(true, *pos);
                        self.track_repeat = Some(TrackRepeat {
                            horizontal: true,
                            pos: *pos,
                            next_in: TRACK_REPEAT_DELAY,
                        });
                        return Handled::Yes;
                    }
                }
                if self.hovered {
                    // Bounds are shifted to visual, so route the raw event —
                    // children hit-test against their (shifted) bounds.
                    route_event(&mut self.base.children, ev)
                } else {
                    Handled::No
                }
            }
            Event::PointerMoved { pos } => {
                self.hovered = vp.contains(*pos);
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
                    self.scroll_to((frac * max_off) as f32);
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
                    self.scroll_to_x((frac * max_off) as f32);
                    Handled::Yes
                } else if let Some(tr) = &mut self.track_repeat {
                    // A held track press is a grab: track the cursor (the repeat
                    // pages toward wherever it is now) and consume the move.
                    tr.pos = *pos;
                    Handled::Yes
                } else if self.hovered {
                    route_event(&mut self.base.children, ev)
                } else {
                    Handled::No
                }
            }
            Event::PointerReleased { .. } => {
                if self.thumb_grab.take().is_some()
                    || self.h_thumb_grab.take().is_some()
                    || self.track_repeat.take().is_some()
                {
                    // Drag / held track press ended: the thumb may go from bright to
                    // rest if the cursor is no longer over the lane, so repaint.
                    self.base.mark_needs_paint();
                    return Handled::Yes;
                }
                // Route the release to children regardless of hover: a press
                // that started inside should still get its matching release even
                // if the cursor drifted out before release.
                route_event(&mut self.base.children, ev)
            }
            // No `Event::Key` handler on purpose: the widget binds no keys (tmux-style
            // app — plain keys go to the underlying content). Keyboard scrolling is a
            // host concern: the app dispatches prefix-gated, configurable scroll
            // actions that call `scroll_to`/`scroll_by`/`ensure_visible`.
            _ => route_event(&mut self.base.children, ev),
        }
    }

    /// Children first (the default recursion), then the held-track-press repeat:
    /// after [`TRACK_REPEAT_DELAY`] a held press in a scrollbar track keeps paging
    /// toward the cursor every [`TRACK_REPEAT_INTERVAL`] until released. Returns
    /// `true` while a press is held so the host keeps ticking.
    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        let mut fire: Option<(bool, Point)> = None;
        if let Some(tr) = &mut self.track_repeat {
            tr.next_in -= dt;
            if tr.next_in <= 0.0 {
                tr.next_in = TRACK_REPEAT_INTERVAL;
                fire = Some((tr.horizontal, tr.pos));
            }
            animating = true;
        }
        if let Some((horizontal, pos)) = fire {
            self.page_toward(horizontal, pos);
        }
        animating
    }
}

impl LayoutExt for ScrollRegion {}
impl StyleExt for ScrollRegion {}
impl Parent for ScrollRegion {}

#[cfg(test)]
mod tests {
    use super::*;

    fn region_with_children(child_heights: &[f64]) -> ScrollRegion {
        // Children are real components only for layout; here we just need bounds
        // set on them. We build a ScrollRegion and manually stamp child bounds
        // (as the layout engine would) so the geometry helpers are testable in
        // isolation. `applied_offset` starts at 0, so bounds are "natural".
        let mut r = ScrollRegion::new();
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 100.0));
        r.base.children.clear();
        let mut y = 0.0;
        for &h in child_heights {
            let mut child = crate::widgets::Flex::column();
            child.base_mut().bounds =
                Rectangle::new(Point::new(0.0, y), Size::new(200.0, h));
            r.base.children.push(Box::new(child));
            y += h;
        }
        r
    }

    #[test]
    fn content_extent_sums_children_and_clamps_to_viewport() {
        let r = region_with_children(&[60.0, 60.0]);
        assert!((r.content_extent() - 120.0).abs() < f64::EPSILON);
        assert!((r.max_offset() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn no_thumb_when_content_fits() {
        let r = region_with_children(&[40.0, 40.0]);
        assert!(r.thumb_rect().is_none());
        assert!(r.max_offset().abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_appears_and_scales_with_visible_fraction() {
        let r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        assert!((t.size.h - (100.0 / 120.0 * 100.0)).abs() < 1e-6);
        assert!(t.loc.y.abs() < 1e-6);
    }

    #[test]
    fn thumb_hit_lane_is_wider_than_the_visible_thumb_and_contains_it() {
        // Regression guard for "thumb misses clicks": the grab lane must be wider
        // than the thin visible thumb and must contain it, so a click near the
        // thumb still grabs.
        let r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        let hit = r.thumb_hit_rect().expect("scrollable → hit lane");
        assert!(hit.size.w > t.size.w, "hit lane is wider than the visible thumb");
        assert_eq!(hit.size.w as i32, THUMB_HIT_W as i32);
        // The visible thumb sits inside the lane horizontally.
        assert!(t.loc.x >= hit.loc.x - 0.001);
        assert!(t.loc.x + t.size.w <= hit.loc.x + hit.size.w + 0.001);
        // Same vertical span.
        assert!((hit.loc.y - t.loc.y).abs() < 1e-6);
        assert!((hit.size.h - t.size.h).abs() < 1e-6);
    }

    #[test]
    fn pressing_in_the_hit_lane_but_off_the_visible_thumb_still_grabs() {
        // A click in the grab padding (left of the thin thumb, inside the wider
        // lane) must still start a drag — the original 8px thumb missed these.
        let mut r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        // A point just left of the visible thumb, inside the hit lane.
        let pos = Point::new(t.loc.x - 4.0, t.loc.y + 4.0);
        assert!(r.thumb_hit_rect().unwrap().contains(pos), "pos is in the hit lane");
        assert!(!t.contains(pos), "pos is NOT on the thin visible thumb");
        let handled = r.event(&Event::PointerPressed { pos });
        assert_eq!(handled, Handled::Yes, "press in the lane grabs the thumb");
        assert!(r.thumb_grab.is_some(), "a drag started");
    }

    #[test]
    fn scroll_to_clamps_to_max_offset() {
        let mut r = region_with_children(&[60.0, 60.0]); // max_offset 20
        assert!((r.scroll_to(50.0) - 20.0_f32).abs() < f32::EPSILON);
        assert!((r.scroll_to(-5.0) - 0.0_f32).abs() < f32::EPSILON);
        assert!((r.scroll_to(10.0) - 10.0_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn sync_shift_bakes_offset_into_children_and_recovers_natural() {
        let mut r = region_with_children(&[60.0, 60.0]); // content 120, vp 100
        r.scroll_offset.set(20.0); // max_offset
        r.sync_shift();
        // applied_offset now equals scroll_offset.
        assert!((r.applied_offset - 20.0).abs() < f64::EPSILON);
        // Children shifted up by 20 (visual position): first child now at y=-20.
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
        // Geometry helpers still report the *natural* extent (120) via the
        // applied_offset recovery — scrolling must not change content_extent.
        assert!((r.content_extent() - 120.0).abs() < f64::EPSILON);
        assert!((r.max_offset() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn on_layout_resets_applied_offset_so_shift_does_not_compound() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_offset.set(20.0);
        r.sync_shift();
        assert!(r.applied_offset.abs() > 0.0); // applied = 20
        // Simulate a fresh layout pass re-stamping natural bounds, then on_layout.
        let mut y = 0.0;
        for c in r.base.children.iter_mut() {
            c.base_mut().bounds = Rectangle::new(Point::new(0.0, y), Size::new(200.0, 60.0));
            y += 60.0;
        }
        r.on_layout();
        // on_layout re-applies the shift immediately (paint is \u0026self and can't
        // sync, so we must leave the bounds already shifted) — no snap to top, no
        // compounding: applied is back to the scroll offset and child 0 is at -20.
        assert!((r.applied_offset - 20.0).abs() < f64::EPSILON);
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
        // A follow-up sync_shift is a no-op (already in sync) — no double shift.
        r.sync_shift();
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn wheel_step_is_viewport_fraction_not_fixed_lines() {
        // Regression guard: the step must scale with the viewport, so a small
        // viewport doesn't jump ~75% per notch (the original overshoot bug).
        let r = region_with_children(&[60.0, 60.0]); // vp 100
        let step = WHEEL_STEP_FRAC * r.base.bounds.size.h;
        // 10% of viewport per line of delta — gentle, proportional.
        assert!((step - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ensure_visible_scrolls_down_when_item_is_below_viewport() {
        let mut r = region_with_children(&[60.0, 60.0]); // vp 100, max 20
        // child[1] natural 60..120, off 0 → visual 60..120, below vp (0..100).
        r.ensure_visible(Rectangle::new(
            Point::new(0.0, 60.0),
            Size::new(200.0, 60.0),
        ));
        // Scrolled so the item's bottom (120) aligns with the viewport bottom
        // (100): offset = 120 - 100 = 20 (= max_offset).
        assert!((r.scroll_offset.get_untracked() - 20.0_f32).abs() < 1e-6);
    }

    #[test]
    fn ensure_visible_scrolls_up_when_item_is_above_viewport() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_to(20.0); // applied_offset=20; child[0] visual y = 0 - 20 = -20
        // child[0] visual rect {y:-20, h:60} — top is above the viewport (y=0).
        r.ensure_visible(Rectangle::new(
            Point::new(0.0, -20.0),
            Size::new(200.0, 60.0),
        ));
        // Scrolled back so the item's top (natural 0) aligns with viewport top.
        assert!(r.scroll_offset.get_untracked().abs() < 1e-6);
    }

    #[test]
    fn ensure_visible_is_noop_when_item_already_visible() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_to(0.0);
        // child[0] natural 0..60 → visual 0..60, fully inside vp 0..100.
        r.ensure_visible(Rectangle::new(
            Point::new(0.0, 0.0),
            Size::new(200.0, 60.0),
        ));
        assert!(r.scroll_offset.get_untracked().abs() < 1e-6, "no scroll when already visible");
    }

    #[test]
    fn scroll_to_child_brings_a_direct_child_into_view() {
        let mut r = region_with_children(&[60.0, 60.0]); // max 20
        r.scroll_to_child(1); // child[1] below → scroll to 20
        assert!((r.scroll_offset.get_untracked() - 20.0_f32).abs() < 1e-6);
        // Out-of-range index is a no-op (no panic, no scroll change).
        let before = r.scroll_offset.get_untracked();
        r.scroll_to_child(99);
        assert!((r.scroll_offset.get_untracked() - before).abs() < f32::EPSILON);
    }

    /// A relayout that GROWS the viewport (resize / zoom-out) must re-clamp a
    /// stale offset — otherwise content stays shifted past the edge with no
    /// scrollbar left to bring it back (the "scrollbar disappears after zoom"
    /// regression).
    #[test]
    fn on_layout_reclamps_a_stale_offset_when_the_viewport_grows() {
        let mut r = region_with_children(&[60.0, 60.0]); // vp 100, content 120
        r.scroll_to(20.0); // at max
        // The viewport grows past the content; layout restamps natural bounds.
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 200.0));
        let mut y = 0.0;
        for c in r.base.children.iter_mut() {
            c.base_mut().bounds = Rectangle::new(Point::new(0.0, y), Size::new(200.0, 60.0));
            y += 60.0;
        }
        r.on_layout();
        // Offset re-clamped to the new max (0): content back at the top, not left
        // shifted off-screen.
        assert!(r.scroll_offset.get_untracked().abs() < f32::EPSILON);
        assert!(r.base.children[0].base().bounds.loc.y.abs() < f64::EPSILON);
    }

    /// A held press in the scrollbar track pages once immediately, then repeats
    /// after the initial delay at the repeat interval, and stops on release.
    #[test]
    fn held_track_press_repeats_paging_until_released() {
        // vp 100 (h), content 600 → max 500; one page = 100.
        let mut r = region_with_children(&[300.0, 300.0]);
        // Press in the vertical track lane, below the thumb.
        let pos = Point::new(195.0, 90.0);
        assert_eq!(r.event(&Event::PointerPressed { pos }), Handled::Yes);
        assert!((r.scroll_offset.get_untracked() - 100.0).abs() < 1e-3, "first page fires on press");
        // Held but before the initial delay: animating, no extra page.
        assert!(r.tick(0.2), "held press keeps the host ticking");
        assert!((r.scroll_offset.get_untracked() - 100.0).abs() < 1e-3);
        // Past the delay: a repeat fires.
        r.tick(0.2);
        assert!((r.scroll_offset.get_untracked() - 200.0).abs() < 1e-3, "repeat after the delay");
        // Steady repeat at the interval.
        r.tick(0.1);
        assert!((r.scroll_offset.get_untracked() - 300.0).abs() < 1e-3, "repeat at the interval");
        // Release disarms it: no further paging however long we tick.
        r.event(&Event::PointerReleased { pos });
        assert!(!r.tick(1.0));
        assert!((r.scroll_offset.get_untracked() - 300.0).abs() < 1e-3, "stopped on release");
    }

    // ── Horizontal axis ──

    /// A region with the given `axes` and child WIDTHS laid left→right in a 100×100
    /// viewport (mirrors `region_with_children`, which stamps heights).
    fn h_region_with_widths(axes: ScrollAxes, child_widths: &[f64]) -> ScrollRegion {
        let mut r = ScrollRegion::new().axes(axes);
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        r.base.children.clear();
        let mut x = 0.0;
        for &w in child_widths {
            let mut child = crate::widgets::Flex::column();
            child.base_mut().bounds = Rectangle::new(Point::new(x, 0.0), Size::new(w, 100.0));
            r.base.children.push(Box::new(child));
            x += w;
        }
        r
    }

    #[test]
    fn horizontal_extent_and_max_offset() {
        let r = h_region_with_widths(ScrollAxes::Both, &[80.0, 80.0]); // content 160, vp 100
        assert!((r.content_extent_x() - 160.0).abs() < f64::EPSILON);
        assert!((r.max_offset_x() - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn horizontal_axis_off_means_no_horizontal_scroll() {
        // A vertical-only region ignores horizontal overflow entirely.
        let r = h_region_with_widths(ScrollAxes::Vertical, &[80.0, 80.0]);
        assert_eq!(r.max_offset_x(), 0.0);
        assert!(r.h_thumb_rect().is_none());
    }

    #[test]
    fn horizontal_thumb_appears_only_when_content_overflows() {
        let fits = h_region_with_widths(ScrollAxes::Horizontal, &[40.0, 40.0]); // 80 ≤ 100
        assert!(fits.h_thumb_rect().is_none());
        let over = h_region_with_widths(ScrollAxes::Horizontal, &[80.0, 80.0]); // 160 > 100
        assert!(over.h_thumb_rect().is_some());
    }

    #[test]
    fn scroll_to_x_clamps_to_bounds() {
        let mut r = h_region_with_widths(ScrollAxes::Both, &[80.0, 80.0]); // max_x 60
        assert!((r.scroll_to_x(1000.0) - 60.0).abs() < f32::EPSILON, "clamped to max");
        assert!((r.scroll_to_x(-5.0)).abs() < f32::EPSILON, "clamped to 0");
    }

    /// Nested regions: the wheel goes to children FIRST, so an inner hovered
    /// scrollable consumes it and the outer region stays put (innermost wins).
    #[test]
    fn nested_region_consumes_the_wheel_before_the_outer_one() {
        // Outer: 200×100 viewport, vertical content 300 (scrollable).
        let mut outer = ScrollRegion::new();
        outer.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 100.0));
        // Inner region at the top of the outer content: 100×50 viewport, content 200.
        let mut inner = ScrollRegion::new();
        inner.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 50.0));
        let mut tall = crate::widgets::Flex::column();
        tall.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 200.0));
        inner.base.children.push(Box::new(tall));
        let inner_offset = inner.scroll_offset();
        outer.base.children.push(Box::new(inner));
        // Filler that makes the OUTER content overflow too.
        let mut filler = crate::widgets::Flex::column();
        filler.base_mut().bounds =
            Rectangle::new(Point::new(0.0, 50.0), Size::new(200.0, 250.0));
        outer.base.children.push(Box::new(filler));

        // Cursor over the inner region (both regions see the move → both hovered).
        let pos = Point::new(10.0, 10.0);
        outer.event(&Event::PointerMoved { pos });
        // Wheel: the inner region must consume it; the outer must not move.
        let handled = outer.event(&Event::Scroll { delta_x: 0.0, delta_y: 1.0 });
        assert_eq!(handled, Handled::Yes);
        assert!(
            inner_offset.get_untracked() > 0.0,
            "inner (hovered) region scrolled"
        );
        assert!(
            outer.scroll_offset.get_untracked().abs() < f32::EPSILON,
            "outer region did not scroll while the inner one consumed the wheel"
        );

        // Cursor over the outer region but OFF the inner one: now the outer scrolls.
        let pos = Point::new(150.0, 80.0);
        outer.event(&Event::PointerMoved { pos });
        let handled = outer.event(&Event::Scroll { delta_x: 0.0, delta_y: 1.0 });
        assert_eq!(handled, Handled::Yes);
        assert!(
            outer.scroll_offset.get_untracked() > 0.0,
            "outer region scrolls when no child consumed the wheel"
        );
    }

    /// A horizontal-only region shows NO vertical thumb even with tall content, and a
    /// horizontal thumb when it overflows — the axes gate both scrollbars.
    #[test]
    fn horizontal_only_region_gates_both_scrollbars() {
        let mut r = ScrollRegion::new().horizontal();
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        let mut child = crate::widgets::Flex::column();
        child.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(300.0, 500.0));
        r.base.children.push(Box::new(child));
        assert!(r.thumb_rect().is_none(), "vertical axis disabled → no vertical thumb");
        assert_eq!(r.max_offset(), 0.0);
        assert!(r.h_thumb_rect().is_some(), "horizontal overflow → horizontal thumb");
    }
}