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
use crate::component::{paint_child, route_event, Base, Component, Event, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

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
    text: String,
    side: TooltipSide,
    delay: f32,
    hovered: Signal<bool>,
    /// Accumulated hover time; the bubble shows once it reaches `delay`.
    elapsed: f32,
}

impl Tooltip {
    /// Wrap `child`, showing `text` on hover.
    pub fn new(child: impl Component + 'static, text: impl Into<String>) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds match it (hover + anchor use them).
        base.style.width = Length::Auto;
        base.style.height = Length::Auto;
        base.children.push(Box::new(child));
        Self {
            base,
            text: text.into(),
            side: TooltipSide::default(),
            delay: DEFAULT_DELAY,
            hovered: signal(false),
            elapsed: 0.0,
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
        self.hovered.get_untracked() && self.elapsed >= self.delay
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
        // The wrapped child first (still fully interactive).
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
        if !self.shown() || self.text.is_empty() {
            return;
        }

        let (surface, accent, glow_c, foreground, ctrl_radius) = {
            let t = cx.theme();
            (t.surface, t.accent, t.glow, t.foreground, t.control_radius())
        };
        let font = self.base.font;
        let chars = self.text.chars().count() as f64;
        let w = chars * (font * MONO_ADVANCE_RATIO) as f64 + 2.0 * PAD_X;
        let h = (font * MONO_LINE_RATIO) as f64 + 2.0 * PAD_Y;
        let rect = self.bubble_rect(self.base.bounds, w, h, cx.viewport());
        let radius = ctrl_radius.min((h / 2.0) as f32);

        // Drawn on the overlay layer so it sits above later siblings.
        cx.with_overlay(|cx| {
            cx.rect(
                rect,
                surface,
                Some(Border { color: accent.with_alpha(180), width: 1.0 }),
                radius,
                Some(Glow { color: glow_c, radius: 5.0, intensity: 0.2 }),
            );
            cx.text(rect, &self.text, foreground, font, TextAlign::Center, false);
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        // Track hover, then route to the child (transparent — never consumes).
        if let Event::PointerMoved { pos } = ev {
            let inside = self.base.bounds.contains(*pos);
            if self.hovered.get_untracked() != inside {
                self.hovered.set(inside);
                if !inside {
                    self.elapsed = 0.0;
                }
            }
        }
        route_event(&mut self.base.children, ev)
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        if self.hovered.get_untracked() {
            // Count up to the reveal delay; the frame it crosses repaints the bubble.
            if self.elapsed < self.delay {
                self.elapsed = (self.elapsed + dt).min(self.delay);
                animating = true;
            }
        } else if self.elapsed > 0.0 {
            self.elapsed = 0.0;
            animating = true; // repaint to hide
        }
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}

impl LayoutExt for Tooltip {}
impl StyleExt for Tooltip {}
impl Parent for Tooltip {}
