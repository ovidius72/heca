//! [`Spinner`] — an indeterminate loading indicator. A display widget animated
//! via [`Component::tick`]. The scene has no rotation primitive, so the spinner
//! is a **ring of dots** (placed with sin/cos) whose brightness sweeps around
//! the circle — a rotating glow without rotating any geometry.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::SignalGet;
use crate::scene::Glow;
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Default diameter (logical px).
const SIZE: f32 = 28.0;
/// Number of dots in the ring.
const DOTS: usize = 8;
/// Seconds for one full revolution of the brightness sweep.
const PERIOD: f32 = 0.9;
/// Dot radius as a fraction of the ring radius.
const DOT_FRAC: f64 = 0.16;
/// Minimum dot alpha (so trailing dots stay faintly visible).
const MIN_ALPHA: f32 = 0.18;

/// An indeterminate spinner.
pub struct Spinner {
    base: Base,
    /// Sweep phase in `0.0..1.0`, advanced by `tick`.
    phase: f32,
}

impl Spinner {
    /// A new spinner at the default size.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.width = Length::Px(SIZE);
        base.style.height = Length::Px(SIZE);
        Self { base, phase: 0.0 }
    }
}

impl Component for Spinner {
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
        let accent = cx.theme().colors.accent;
        let b = self.base.bounds;
        let cx_pt = b.loc.x + b.size.w / 2.0;
        let cy_pt = b.loc.y + b.size.h / 2.0;
        let ring_r = (b.size.w.min(b.size.h) / 2.0) * (1.0 - DOT_FRAC);
        let dot_r = ring_r * DOT_FRAC * 2.0;

        for i in 0..DOTS {
            let frac = i as f32 / DOTS as f32;
            let angle = frac * std::f32::consts::TAU;
            let dx = (angle.cos() as f64) * ring_r;
            let dy = (angle.sin() as f64) * ring_r;

            // Brightness sweeps around: the dot nearest the phase is brightest.
            let mut t = self.phase - frac;
            t -= t.floor();
            let brightness = MIN_ALPHA + (1.0 - MIN_ALPHA) * (1.0 - t);
            let alpha = (brightness.clamp(0.0, 1.0) * 255.0).round() as u8;

            let dot = Rectangle::new(
                Point::new(cx_pt + dx - dot_r / 2.0, cy_pt + dy - dot_r / 2.0),
                Size::new(dot_r, dot_r),
            );
            let glow = Some(Glow {
                color: accent,
                radius: 6.0,
                intensity: 0.12 * brightness,
            });
            cx.rect(
                dot,
                accent.with_alpha(alpha),
                None,
                (dot_r / 2.0) as f32,
                glow,
            );
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        if !self.base.visible.get_untracked() {
            return false;
        }
        self.phase = (self.phase + dt / PERIOD).rem_euclid(1.0);
        // Damage just the spinner's own rect each frame, so its animation doesn't
        // force a whole-scene redraw.
        self.base.mark_needs_paint();
        true
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Spinner {}
