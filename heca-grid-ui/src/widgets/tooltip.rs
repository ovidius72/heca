//! [`Tooltip`] — a hover-revealed floating label for any child.
//!
//! A **transparent wrapper** (like [`KeyHint`](super::KeyHint)): it forwards
//! layout/events to the wrapped child and only *adds paint*. While the pointer
//! rests over the child past a short delay, a small bubble (rounded surface +
//! border + glow + text) is drawn on the scene's **overlay layer** (via
//! [`PaintCx::with_overlay`](crate::component::PaintCx::with_overlay)) so it sits
//! above later siblings and is never clipped. It captures **no** input — a
//! tooltip never eats clicks — so it does not use the `overlay_active` path that
//! input-grabbing popovers (`Select`) do.
//!
//! Placement is **viewport-aware on all four sides**: the bubble centers on the
//! chosen [`TooltipSide`] of the target, but flips to the opposite side when there
//! isn't room (`Top`↔`Bottom`, `Left`↔`Right`), and its cross-axis is clamped to
//! the viewport so it never spills off-screen.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{
    paint_child, route_event, soonest_redraw, Base, Component, Event, Handled, PaintCx,
};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::{Glow, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;
use std::time::Instant;

/// Smallest rect containing both `a` and `b` (child bounds ∪ bubble rect).
fn union(a: Rectangle, b: Rectangle) -> Rectangle {
    let x0 = a.loc.x.min(b.loc.x);
    let y0 = a.loc.y.min(b.loc.y);
    let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
    let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
    Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
}

/// Which side of the target the bubble appears on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TooltipSide {
    /// Above the target (flips to `Bottom` if there's no room).
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Seconds the pointer must rest before the bubble shows.
const DEFAULT_DELAY: f32 = 0.5;
/// Gap between the target and the bubble (logical px).
const GAP: f64 = 6.0;
/// Bubble inner padding.
const PAD_X: f64 = 8.0;
const PAD_Y: f64 = 5.0;

/// A transparent wrapper that reveals a floating label on hover.
pub struct Tooltip {
    base: Base,
    text: Signal<String>,
    seen_text: String,
    side: TooltipSide,
    delay: f32,
    /// When the pointer entered the child (`None` = not hovering). The bubble shows
    /// once this is `delay` seconds in the past. Wall-clock (like the `Input` caret)
    /// so the host can sleep through the delay and wake once, rather than ticking
    /// every frame — see [`next_redraw`](Component::next_redraw).
    hover_since: Option<Instant>,
    /// Viewport cached at paint so `tick`/`damage_bounds` can place the bubble.
    viewport: Cell<Size>,
    /// Last-painted bubble visibility, to detect show/hide transitions in `tick`.
    last_shown: Cell<bool>,
}

impl Tooltip {
    /// Wrap `child`, showing `text` on hover.
    pub fn new(child: impl Component + 'static, text: impl Into<String>) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds match it (hover + anchor use them).
        base.style.width = Length::Auto;
        base.style.height = Length::Auto;
        base.children.push(Box::new(child));
        let text = signal(text.into());
        Self {
            base,
            seen_text: text.get_untracked(),
            text,
            side: TooltipSide::default(),
            delay: DEFAULT_DELAY,
            hover_since: None,
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            last_shown: Cell::new(false),
        }
    }

    /// Wrap `child`, showing reactive `text` on hover.
    pub fn new_signal(child: impl Component + 'static, text: Signal<String>) -> Self {
        let mut base = Base::new();
        base.style.width = Length::Auto;
        base.style.height = Length::Auto;
        base.children.push(Box::new(child));
        Self {
            base,
            seen_text: text.get_untracked(),
            text,
            side: TooltipSide::default(),
            delay: DEFAULT_DELAY,
            hover_since: None,
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            last_shown: Cell::new(false),
        }
    }

    /// Which side of the target to anchor to (default [`TooltipSide::Top`]).
    pub fn side(mut self, side: TooltipSide) -> Self {
        self.side = side;
        self
    }

    /// Hover delay before the bubble appears, in seconds (default `0.5`).
    pub fn delay(mut self, seconds: f32) -> Self {
        self.delay = seconds.max(0.0);
        self
    }

    fn shown(&self) -> bool {
        self.hover_since
            .is_some_and(|since| since.elapsed().as_secs_f32() >= self.delay)
    }

    /// The bubble's text size (logical px) from the resolved font + label length.
    fn bubble_size(&self) -> (f64, f64) {
        let font = self.base.font;
        let chars = self.text.get_untracked().chars().count() as f64;
        let w = chars * (font * MONO_ADVANCE_RATIO) as f64 + 2.0 * PAD_X;
        let h = (font * MONO_LINE_RATIO) as f64 + 2.0 * PAD_Y;
        (w, h)
    }

    /// Where the bubble would draw for the cached viewport (`None` if no text).
    /// Used to damage the right region on show/hide — the bubble sits off our own
    /// bounds, on the overlay layer.
    fn current_bubble_rect(&self) -> Option<Rectangle> {
        if self.text.get_untracked().is_empty() {
            return None;
        }
        let (w, h) = self.bubble_size();
        Some(self.bubble_rect(self.base.bounds, w, h, self.viewport.get()))
    }

    /// Whether a `w×h` bubble fits on `side` of target `b` within viewport `vp`.
    fn fits(side: TooltipSide, b: Rectangle, w: f64, h: f64, vp: Size) -> bool {
        match side {
            TooltipSide::Top => b.loc.y - h - GAP >= 0.0,
            TooltipSide::Bottom => b.loc.y + b.size.h + h + GAP <= vp.h,
            TooltipSide::Left => b.loc.x - w - GAP >= 0.0,
            TooltipSide::Right => b.loc.x + b.size.w + w + GAP <= vp.w,
        }
    }

    /// The bubble rect for `w×h` text, anchored to target `b` and made
    /// **space-aware**: the preferred [`side`](Tooltip::side) flips to its opposite
    /// when there's no room (all four sides), and the cross-axis is clamped to the
    /// viewport so the bubble never spills off-screen.
    fn bubble_rect(&self, b: Rectangle, w: f64, h: f64, vp: Size) -> Rectangle {
        // Flip to the opposite side if the preferred one doesn't fit but it does.
        let opposite = match self.side {
            TooltipSide::Top => TooltipSide::Bottom,
            TooltipSide::Bottom => TooltipSide::Top,
            TooltipSide::Left => TooltipSide::Right,
            TooltipSide::Right => TooltipSide::Left,
        };
        let side = if Self::fits(self.side, b, w, h, vp) || !Self::fits(opposite, b, w, h, vp) {
            self.side
        } else {
            opposite
        };

        let (mut x, mut y) = match side {
            TooltipSide::Top => (b.loc.x + (b.size.w - w) / 2.0, b.loc.y - h - GAP),
            TooltipSide::Bottom => (b.loc.x + (b.size.w - w) / 2.0, b.loc.y + b.size.h + GAP),
            TooltipSide::Left => (b.loc.x - w - GAP, b.loc.y + (b.size.h - h) / 2.0),
            TooltipSide::Right => (b.loc.x + b.size.w + GAP, b.loc.y + (b.size.h - h) / 2.0),
        };
        // Clamp the cross-axis (the one the side doesn't pin) into the viewport.
        match side {
            TooltipSide::Top | TooltipSide::Bottom if vp.w.is_finite() => {
                x = x.clamp(0.0, (vp.w - w).max(0.0));
            }
            TooltipSide::Left | TooltipSide::Right if vp.h.is_finite() => {
                y = y.clamp(0.0, (vp.h - h).max(0.0));
            }
            _ => {}
        }
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }
}

impl Component for Tooltip {
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
        // Cache the viewport so `tick`/`damage_bounds` place the bubble identically.
        self.viewport.set(cx.viewport());
        // The wrapped child first (still fully interactive).
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
        let text = self.text.get_untracked();
        if !self.shown() || text.is_empty() {
            return;
        }

        let (surface, accent, glow_c, foreground, ctrl_radius) = {
            let t = cx.theme();
            (t.surface, t.accent, t.glow, t.foreground, t.control_radius())
        };
        let font = self.base.font;
        let (w, h) = self.bubble_size();
        let rect = self.bubble_rect(self.base.bounds, w, h, cx.viewport());
        let radius = ctrl_radius.min((h / 2.0) as f32);

        // Drawn on the overlay layer so it sits above later siblings.
        cx.with_overlay(|cx| {
            let border = cx.border(accent.with_alpha(180));
            cx.rect(
                rect,
                surface,
                border,
                radius,
                Some(Glow { color: glow_c, radius: 5.0, intensity: 0.2 }),
            );
            cx.text(rect, &text, foreground, font, TextAlign::Center, false);
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        // Track hover, then route to the child (transparent — never consumes).
        if let Event::PointerMoved { pos } = ev {
            let inside = self.base.bounds.contains(*pos);
            if inside != self.hover_since.is_some() {
                // Enter starts the reveal clock; leave clears it (the next `tick`
                // detects the show/hide transition and damages the bubble).
                self.hover_since = inside.then(Instant::now);
            }
        }
        route_event(&mut self.base.children, ev)
    }

    fn tick(&mut self, dt: f32) -> bool {
        let next_text = self.text.get_untracked();
        if next_text != self.seen_text {
            self.seen_text = next_text;
            self.base.mark_needs_paint();
        }
        // The bubble's reveal is timed (wall-clock), not a continuous animation:
        // repaint only when it crosses the show/hide boundary, and damage just the
        // bubble (via `damage_bounds`) instead of forcing a full frame.
        let now_shown = self.shown();
        if now_shown != self.last_shown.get() {
            self.last_shown.set(now_shown);
            self.base.mark_needs_paint();
        }
        // Children still animate normally (they mark their own rects).
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    fn next_redraw(&self) -> Option<f32> {
        // While hovering pre-reveal, wake the host exactly when the bubble appears.
        let mut soonest = self.hover_since.and_then(|since| {
            let e = since.elapsed().as_secs_f32();
            (e < self.delay).then_some(self.delay - e)
        });
        for child in self.base.children.iter() {
            soonest = soonest_redraw(soonest, child.next_redraw());
        }
        soonest
    }

    fn damage_bounds(&self) -> Rectangle {
        // The bubble draws on the overlay layer, offset from our own bounds, so a
        // show/hide repaint must cover it (plus our bounds, harmlessly).
        match self.current_bubble_rect() {
            Some(bubble) => union(self.base.bounds, bubble),
            None => self.base.bounds,
        }
    }
}

impl LayoutExt for Tooltip {}
impl StyleExt for Tooltip {}
impl Parent for Tooltip {}
