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
//! v1 is vertical-only; the thumb is a theme-**accent** grip that brightens on
//! hover/drag (mirroring [`MarkerGroup`](crate::widgets::MarkerGroup)'s grip bar)
//! and sits in a wider invisible grab lane so a thin thumb is easy to click. A
//! distinct scrollbar color token and horizontal scrolling are future work.

use crate::builders::{LayoutExt, Parent};
use crate::component::{
    paint_child, route_event, Base, Component, Event, GridKey, Handled, PaintCx,
};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::style::Direction;
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
/// Thumb alpha at rest — dim accent (reads as "there's more content").
const THUMB_REST_ALPHA: u8 = 90;
/// Thumb alpha when hovered or dragged — brightened to read as grabbable, the
/// same affordance as [`MarkerGroup`](crate::widgets::MarkerGroup)'s grip bar.
const THUMB_HOVER_ALPHA: u8 = 200;
/// Wheel step as a fraction of the viewport height per "line" of delta. The
/// winit wheel delta is already in lines, so one notch (delta ≈ 1) scrolls ~10%
/// of the viewport — gentle in a small sidebar, scales up for a tall one. (The
/// previous build multiplied by a fixed line count × font, which made each notch
/// jump ~75% of a small viewport and overshoot.)
const WHEEL_STEP_FRAC: f64 = 0.1;
/// Keyboard scroll step as a fraction of the viewport per press. Matches the
/// wheel step so keyboard and wheel feel consistent (one arrow/j/k press ≈ one
/// wheel notch). `Home`/`End` jump to top/bottom; PageUp/PageDown are future work
/// (`GridKey` has no page keys yet — they'd need adding to the enum + host
/// mapping).
const KEY_STEP_FRAC: f64 = 0.1;

/// An embeddable vertical scroll viewport hosting a column of children.
///
/// Build with [`ScrollRegion::new`], append children via [`Parent::child`], and
/// read/drive the position via [`ScrollRegion::scroll_offset`] /
/// [`ScrollRegion::scroll_to`]. The scrollbar appears automatically when the
/// content is taller than the viewport.
///
/// **Wheel gating.** `Event::Scroll` carries no position, so the default
/// broadcast router (`route_event`) can't hit-test it — an inline scroll region
/// would swallow *every* wheel event in the tree. To avoid that, the region
/// tracks whether the cursor is over it via `PointerMoved` and only consumes a
/// scroll when hovered (and scrollable). This works for a single inline region;
/// *nested* scroll regions need host-side hit-testing (finding the innermost
/// scrollable under the cursor), which is future work.
pub struct ScrollRegion {
    base: Base,
    /// Vertical scroll offset (content px shifted up). 0 = top.
    scroll_offset: Signal<f32>,
    /// The shift currently baked into the children's bounds (= `scroll_offset`
    /// at the last [`sync_shift`](Self::sync_shift)). Bounds hold `natural -
    /// applied_offset`; geometry helpers recover natural as `bounds +
    /// applied_offset`. Reset to 0 by [`on_layout`](Component::on_layout) when
    /// layout re-computes bounds to natural.
    applied_offset: f64,
    /// While dragging the thumb: the y-offset (content px) from the thumb's top
    /// where the grab landed, so the grab point stays under the cursor. `None`
    /// when not dragging.
    thumb_grab: Option<f64>,
    /// Whether the cursor is currently over this region. Updated from
    /// `PointerMoved`/`PointerPressed`; gates `Event::Scroll` so an inline
    /// region only swallows the wheel when actually hovered.
    hovered: bool,
    /// Whether the cursor is over the scrollbar thumb's grab lane. Drives the
    /// hover affordance (the thumb brightens, like [`MarkerGroup`](crate::widgets::MarkerGroup)'s
    /// grip bar).
    thumb_hovered: bool,
}

impl ScrollRegion {
    /// A new vertical scroll region.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Column;
        Self {
            base,
            scroll_offset: signal(0.0),
            applied_offset: 0.0,
            thumb_grab: None,
            hovered: false,
            thumb_hovered: false,
        }
    }

    /// The reactive scroll offset (content px). Read or drive it from the host:
    /// `region.scroll_offset().get_untracked()` / `.set(v)`. Use
    /// [`scroll_to`](Self::scroll_to) to set with clamping.
    pub fn scroll_offset(&self) -> Signal<f32> {
        self.scroll_offset
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
    /// `[0, max_offset]`. The keyboard handler and wheel both go through here.
    fn scroll_by(&mut self, delta: f64) {
        let next = self.scroll_offset.get_untracked() as f64 + delta;
        self.scroll_to(next as f32);
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

    /// Largest valid offset: `content_extent − viewport_h` (≥ 0).
    fn max_offset(&self) -> f64 {
        (self.content_extent() - self.base.bounds.size.h).max(0.0)
    }

    /// The scrollbar thumb rect (in viewport space), or `None` when the content
    /// fits (no scroll).
    fn thumb_rect(&self) -> Option<Rectangle> {
        let vp = self.base.bounds;
        let content_h = self.content_extent();
        if content_h <= vp.size.h + 0.5 {
            return None;
        }
        let track_h = vp.size.h;
        let thumb_h = ((vp.size.h / content_h) * track_h).max(MIN_THUMB);
        let max_off = content_h - vp.size.h;
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

    /// Bake the current `scroll_offset` into the children's bounds. Shifts each
    /// direct child's subtree by `applied_offset − scroll_offset` so the bounds
    /// end at `natural − scroll_offset` (the visual position). Idempotent when
    /// already in sync. Called from `paint` and `event` so bounds are always
    /// current for drawing, hit-testing, and DnD.
    fn sync_shift(&mut self) {
        let target = self.scroll_offset.get_untracked() as f64;
        let delta = self.applied_offset - target;
        if delta != 0.0 {
            for child in self.base.children.iter_mut() {
                shift_subtree(child.as_mut(), 0.0, delta);
            }
            self.applied_offset = target;
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

    /// Participates in keyboard focus so the host can focus it (click or Tab)
    /// and deliver scroll keys (`Event::Key` goes to the focused component only).
    fn focusable(&self) -> bool {
        true
    }

    /// Layout just re-computed every bound to its natural position — clear the
    /// baked shift so the next `sync_shift` re-applies it from scratch instead
    /// of compounding. (Post-order: children already assigned.)
    fn on_layout(&mut self) {
        self.applied_offset = 0.0;
        self.sync_shift();
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let vp = self.base.bounds;
        // Keep the baked shift current (paint takes `&self`, so sync via the
        // signal value; the shift was already applied by the last `event`/layout
        // reset — `applied_offset` matches `scroll_offset` here in steady state).
        cx.with_clip(vp, |cx| {
            for child in &self.base.children {
                paint_child(child.as_ref(), cx);
            }
        });
        // Focus ring (keyboard focus only — focus-visible) so the user sees
        // which region receives scroll keys. Mirrors Button/Input's focus
        // affordance; drawn in viewport space (not clipped, not scrolled).
        if self.base.focus_visible.get_untracked() && cx.theme().show_focus_border {
            cx.corner_brackets(vp, cx.theme().accent);
        }

        // Scrollbar thumb on top, in viewport space (not scrolled with content).
        // Theme-driven hover affordance (mirrors MarkerGroup's grip bar): the
        // thumb is a dim accent at rest, brightens when its grab lane is hovered,
        // and is full-bright while dragged — reading as "grab here". Radius and
        // color both come from the theme (no hardcoded radius/Color).
        if let Some(t) = self.thumb_rect() {
            let alpha = if self.thumb_grab.is_some() || self.thumb_hovered {
                THUMB_HOVER_ALPHA
            } else {
                THUMB_REST_ALPHA
            };
            let theme = cx.theme();
            let color = theme.accent.with_alpha(alpha);
            cx.rect(t, color, None, theme.control_radius(), None);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        // Bake the current scroll offset into bounds first, so hit-testing and
        // DnD see the visual position (bounds === what's drawn).
        self.sync_shift();
        let vp = self.base.bounds;

        match ev {
            Event::Scroll { delta } => {
                // Only swallow the wheel when the cursor is over this region AND it
                // is scrollable. `Event::Scroll` has no position, so the router
                // can't hit-test it; `hovered` (from `PointerMoved`) is our gate.
                // Otherwise let it propagate so the host page (or a nested region)
                // can scroll.
                if self.hovered && self.max_offset() > 0.0 {
                    let step = WHEEL_STEP_FRAC * vp.size.h;
                    let next = self.scroll_offset.get_untracked() as f64 + (*delta as f64) * step;
                    self.scroll_to(next as f32);
                    Handled::Yes
                } else {
                    route_event(&mut self.base.children, ev)
                }
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
                } else if self.hovered {
                    route_event(&mut self.base.children, ev)
                } else {
                    Handled::No
                }
            }
            Event::PointerReleased { .. } => {
                if self.thumb_grab.take().is_some() {
                    // Drag ended: the thumb may go from drag-bright to rest if the
                    // cursor is no longer over the lane, so repaint.
                    self.base.mark_needs_paint();
                    return Handled::Yes;
                }
                // Route the release to children regardless of hover: a press
                // that started inside should still get its matching release even
                // if the cursor drifted out before release.
                route_event(&mut self.base.children, ev)
            }
            Event::Key { key, pressed: true } => {
                // `Event::Key` is delivered to the focused component only
                // (FocusManager), so reaching here means this region is focused;
                // a focused *child* would receive its keys directly via its own
                // `event`, never through us. Scroll keys move the viewport by a
                // fraction of its height (matching the wheel step); `Home`/`End`
                // jump to top/bottom. `j`/`k` match with or without Ctrl, so both
                // `j`/`k` and `Ctrl+j`/`Ctrl+k` scroll (Ctrl+K is host-bound to the
                // command palette in the showcase, so it won't reach here - use
                // `k`/`Ctrl+J`/arrows there; the chord is configurable in the app).
                if !self.base.focused.get_untracked() {
                    return Handled::No;
                }
                let step = KEY_STEP_FRAC * self.base.bounds.size.h;
                match *key {
                    GridKey::ArrowUp | GridKey::Char('k') => {
                        self.scroll_by(-step);
                        Handled::Yes
                    }
                    GridKey::ArrowDown | GridKey::Char('j') => {
                        self.scroll_by(step);
                        Handled::Yes
                    }
                    GridKey::Home => {
                        self.scroll_to(0.0);
                        Handled::Yes
                    }
                    GridKey::End => {
                        self.scroll_to(self.max_offset() as f32);
                        Handled::Yes
                    }
                    _ => Handled::No,
                }
            }
            _ => route_event(&mut self.base.children, ev),
        }
    }
}

/// Shift a component's subtree's bounds by `(dx, dy)` — the whole-page scroll
/// pattern (`offset_tree`), applied here to a scroll region's children.
fn shift_subtree(c: &mut dyn Component, dx: f64, dy: f64) {
    c.base_mut().bounds.loc.x += dx;
    c.base_mut().bounds.loc.y += dy;
    let n = c.base().children.len();
    for i in 0..n {
        let child = &mut c.base_mut().children[i];
        shift_subtree(child.as_mut(), dx, dy);
    }
}


impl LayoutExt for ScrollRegion {}
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
    fn keyboard_arrows_jk_scroll_when_focused() {
        // Focus-gated keyboard scroll: ArrowDown/j move down by KEY_STEP_FRAC * vp,
        // ArrowUp/k move up, Home/End jump to top/bottom. Keys are delivered to
        // the focused component only, so the gate is `base.focused`.
        let mut r = region_with_children(&[60.0, 60.0]); // vp 100, max_offset 20
        r.base.focused.set(true);
        let step = KEY_STEP_FRAC * r.base.bounds.size.h; // 10

        // Not focused → keys do nothing (gate).
        r.base.focused.set(false);
        assert_eq!(
            r.event(&Event::Key { key: GridKey::ArrowDown, pressed: true }),
            Handled::No
        );
        assert!(r.scroll_offset.get_untracked().abs() < f32::EPSILON);

        // Focused → ArrowDown scrolls one step.
        r.base.focused.set(true);
        assert_eq!(
            r.event(&Event::Key { key: GridKey::ArrowDown, pressed: true }),
            Handled::Yes
        );
        assert!((r.scroll_offset.get_untracked() - step as f32).abs() < 1e-6);

        // `j` scrolls down another step (same as ArrowDown, with or without Ctrl).
        assert_eq!(
            r.event(&Event::Key { key: GridKey::Char('j'), pressed: true }),
            Handled::Yes
        );
        assert!((r.scroll_offset.get_untracked() - 2.0 * step as f32).abs() < 1e-6);

        // `k` scrolls up one step.
        assert_eq!(
            r.event(&Event::Key { key: GridKey::Char('k'), pressed: true }),
            Handled::Yes
        );
        assert!((r.scroll_offset.get_untracked() - step as f32).abs() < 1e-6);

        // ArrowUp scrolls up to the top (clamped at 0).
        assert_eq!(
            r.event(&Event::Key { key: GridKey::ArrowUp, pressed: true }),
            Handled::Yes
        );
        assert!(r.scroll_offset.get_untracked().abs() < f32::EPSILON);

        // End jumps to max_offset (20).
        assert_eq!(
            r.event(&Event::Key { key: GridKey::End, pressed: true }),
            Handled::Yes
        );
        assert!((r.scroll_offset.get_untracked() - 20.0_f32).abs() < 1e-6);

        // Home jumps back to 0.
        assert_eq!(
            r.event(&Event::Key { key: GridKey::Home, pressed: true }),
            Handled::Yes
        );
        assert!(r.scroll_offset.get_untracked().abs() < f32::EPSILON);

        // A non-scroll key is not consumed (falls through to the host).
        assert_eq!(
            r.event(&Event::Key { key: GridKey::Char('x'), pressed: true }),
            Handled::No
        );
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
}