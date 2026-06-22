//! [`ScrollRegion`] — an embeddable vertical scroll viewport.
//!
//! A scrollable column: children are laid out top-to-bottom at their natural
//! height (the layout engine never flex-shrinks them, so the column overflows),
//! and the visible window is the [`ScrollRegion`]'s own bounds. Content beyond
//! the viewport is clipped (renderer `PushClip`/`PopClip`) and painted shifted
//! up by `-scroll_offset` (renderer `Translate`/`PopTranslate` via
//! [`PaintCx::with_offset`](crate::component::PaintCx::with_offset)).
//!
//! Interaction: the wheel (`Event::Scroll`) advances the offset (clamped to
//! `[0, max_offset]`), and the auto-shown scrollbar thumb is draggable. Pointer
//! coordinates are translated into content space before being routed to
//! children, so buttons/items inside a scrolled list remain clickable at their
//! *visual* position. The offset is also exposed as a reactive
//! [`Signal<f32>`](crate::reactive::Signal) the host can read or drive directly.
//!
//! v1 is vertical-only and the thumb is theme-colored (`muted`); a distinct
//! scrollbar color token and horizontal scrolling are future work.

use crate::builders::{LayoutExt, Parent};
use crate::color::Color;
use crate::component::{paint_child, route_event, Base, Component, Event, Handled, PaintCx};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::style::Direction;
use heca_core::layout::{Point, Rectangle, Size};

/// Scrollbar thumb width (logical px).
const SCROLLBAR_W: f64 = 8.0;
/// Gap between the scrollbar and the content edge.
const SCROLLBAR_PAD: f64 = 2.0;
/// Minimum thumb height so a very long list still has a grabbable thumb.
const MIN_THUMB: f64 = 24.0;
/// Wheel delta is in "lines"; multiply by this many font-sized steps.
const WHEEL_STEP_LINES: f32 = 3.0;

/// An embeddable vertical scroll viewport hosting a column of children.
///
/// Build with [`ScrollRegion::new`], append children via [`Parent::child`], and
/// read/drive the position via [`ScrollRegion::scroll_offset`] /
/// [`ScrollRegion::scroll_to`]. The scrollbar appears automatically when the
/// content is taller than the viewport.
pub struct ScrollRegion {
    base: Base,
    /// Vertical scroll offset (content px shifted up). 0 = top.
    scroll_offset: Signal<f32>,
    /// While dragging the thumb: the y-offset (content px) from the thumb's top
    /// where the grab landed, so the grab point stays under the cursor. `None`
    /// when not dragging.
    thumb_grab: Option<f64>,
}

impl ScrollRegion {
    /// A new vertical scroll region.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Column;
        Self {
            base,
            scroll_offset: signal(0.0),
            thumb_grab: None,
        }
    }

    /// The reactive scroll offset (content px). Read or drive it from the host:
    /// `region.scroll_offset().get_untracked()` / `.set(v)`. Use
    /// [`scroll_to`](Self::scroll_to) to set with clamping.
    pub fn scroll_offset(&self) -> Signal<f32> {
        self.scroll_offset
    }

    /// Set the scroll offset, clamped to `[0, max_offset]`, and request a
    /// repaint. Returns the clamped value actually applied.
    pub fn scroll_to(&self, offset: f32) -> f32 {
        let max = self.max_offset() as f32;
        let v = offset.clamp(0.0, max);
        self.scroll_offset.set(v);
        self.base.mark_needs_paint();
        v
    }

    /// Total content extent along the scroll axis (max child bottom relative to
    /// this region's top, never less than the viewport height).
    fn content_extent(&self) -> f64 {
        let vp = self.base.bounds;
        let mut max_bottom = vp.loc.y + vp.size.h;
        for c in &self.base.children {
            let b = c.base().bounds;
            let bottom = b.loc.y + b.size.h;
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

    /// The scrollbar thumb rect, or `None` when the content fits (no scroll).
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

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let vp = self.base.bounds;
        let offset = self.scroll_offset.get_untracked() as f64;
        // Clip to the viewport, then shift content up by the offset. Clip rects
        // pushed before the offset stay untranslated (the viewport window);
        // inner clips move with the content.
        cx.with_clip(vp, |cx| {
            cx.with_offset(Point::new(0.0, -offset), |cx| {
                for child in &self.base.children {
                    paint_child(child.as_ref(), cx);
                }
            });
        });
        // Scrollbar thumb on top, not translated (it lives in viewport space).
        if let Some(t) = self.thumb_rect() {
            let color = scrollbar_thumb_color(cx);
            cx.rect(t, color, None, (SCROLLBAR_W / 2.0) as f32, None);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        let vp = self.base.bounds;
        let offset = self.scroll_offset.get_untracked() as f64;

        match ev {
            Event::Scroll { delta } => {
                if self.max_offset() > 0.0 {
                    let step = WHEEL_STEP_LINES * self.base.font;
                    let next = (offset + (*delta as f64) * step as f64) as f32;
                    self.scroll_to(next);
                    Handled::Yes
                } else {
                    // Not scrollable here — let a (possibly scrollable) child have it.
                    route_event(&mut self.base.children, ev)
                }
            }
            Event::PointerPressed { pos } => {
                if let Some(t) = self.thumb_rect()
                    && t.contains(*pos)
                {
                    // Grab the thumb: remember where in the thumb the press
                    // landed so the grab point tracks the cursor.
                    self.thumb_grab = Some(pos.y - t.loc.y);
                    return Handled::Yes;
                }
                self.route_to_children_translated(ev, offset)
            }
            Event::PointerMoved { pos } => {
                if let Some(grab) = self.thumb_grab {
                    // Drag: derive offset from the thumb top under the cursor.
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
                    return Handled::Yes;
                }
                self.route_to_children_translated(ev, offset)
            }
            Event::PointerReleased { .. } => {
                if self.thumb_grab.take().is_some() {
                    return Handled::Yes;
                }
                self.route_to_children_translated(ev, offset)
            }
            _ => route_event(&mut self.base.children, ev),
        }
    }
}

impl ScrollRegion {
    /// Route a pointer event to children with the position translated into
    /// content space (children's bounds are absolute/untranslated, while they
    /// are *painted* shifted up by `offset`; a click at the visual spot must be
    /// mapped back by `+offset` to hit the right child).
    fn route_to_children_translated(&mut self, ev: &Event, offset: f64) -> Handled {
        let translated = match ev {
            Event::PointerMoved { pos } => {
                Event::PointerMoved { pos: shift_y(*pos, offset) }
            }
            Event::PointerPressed { pos } => {
                Event::PointerPressed { pos: shift_y(*pos, offset) }
            }
            Event::PointerReleased { pos } => {
                Event::PointerReleased { pos: shift_y(*pos, offset) }
            }
            other => *other,
        };
        route_event(&mut self.base.children, &translated)
    }
}

/// Return `pos` shifted down by `offset` (visual → content space).
fn shift_y(pos: Point, offset: f64) -> Point {
    Point::new(pos.x, pos.y + offset)
}

/// Thumb color: the theme's `muted` token, slightly lifted for visibility.
fn scrollbar_thumb_color(cx: &PaintCx) -> Color {
    cx.theme().muted
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
        // isolation.
        let mut r = ScrollRegion::new();
        // Simulate layout: viewport at (0,0), 200 wide × 100 tall.
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
        // Two 60px children = 120px content; viewport 100 → extent 120.
        let r = region_with_children(&[60.0, 60.0]);
        assert!((r.content_extent() - 120.0).abs() < f64::EPSILON);
        assert!((r.max_offset() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn no_thumb_when_content_fits() {
        // Children total 80px < viewport 100 → not scrollable.
        let r = region_with_children(&[40.0, 40.0]);
        assert!(r.thumb_rect().is_none());
        assert!(r.max_offset().abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_appears_and_scales_with_visible_fraction() {
        let r = region_with_children(&[60.0, 60.0]); // content 120, viewport 100
        let t = r.thumb_rect().expect("scrollable → thumb");
        // thumb_h = viewport/content * track = 100/120*100 ≈ 83.3
        assert!((t.size.h - (100.0 / 120.0 * 100.0)).abs() < 1e-6);
        // At offset 0 the thumb sits at the top.
        assert!(t.loc.y.abs() < 1e-6);
    }

    #[test]
    fn scroll_to_clamps_to_max_offset() {
        let r = region_with_children(&[60.0, 60.0]); // max_offset 20
        assert!((r.scroll_to(50.0) - 20.0_f32).abs() < f32::EPSILON);
        assert!((r.scroll_to(-5.0) - 0.0_f32).abs() < f32::EPSILON);
        assert!((r.scroll_to(10.0) - 10.0_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn shift_y_maps_visual_to_content_space() {
        let p = Point::new(5.0, 7.0);
        let s = shift_y(p, 20.0);
        assert!((s.x - 5.0).abs() < f64::EPSILON);
        assert!((s.y - 27.0).abs() < f64::EPSILON);
    }
}