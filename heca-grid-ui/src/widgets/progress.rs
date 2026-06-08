//! [`ProgressBar`] — a determinate progress track whose accent fill animates
//! toward a `0.0..=1.0` value. A display widget: bind it to a `Signal<f32>` via
//! [`value`](ProgressBar::value) / [`set`](ProgressBar::set); the fill eases to
//! the target over time in [`Component::tick`].

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, Glow};
use crate::style::Length;
use heca_core::layout::{Rectangle, Size};

/// Default track width (logical px).
const DEFAULT_WIDTH: f32 = 240.0;
/// Track height (logical px).
const HEIGHT: f32 = 8.0;
/// Seconds for the fill to ease a full 0→1 sweep.
const ANIM_DURATION: f32 = 0.25;
/// Fill glow radius (px).
const GLOW_RADIUS: f32 = 12.0;
/// Fill glow intensity.
const GLOW_INTENSITY: f32 = 0.12;

/// A determinate progress bar.
pub struct ProgressBar {
    base: Base,
    /// Target value in `0.0..=1.0`, exposed reactively via [`state`](ProgressBar::state).
    value: Signal<f32>,
    /// Animated displayed fraction, eased toward `value`.
    shown: f32,
}

impl ProgressBar {
    /// A new bar at 0.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.width = Length::Px(DEFAULT_WIDTH);
        base.style.height = Length::Px(HEIGHT);
        Self {
            base,
            value: signal(0.0),
            shown: 0.0,
        }
    }

    /// Set the initial value (clamped to `0.0..=1.0`, no animation).
    pub fn value(mut self, value: f32) -> Self {
        let v = value.clamp(0.0, 1.0);
        self.value.set(v);
        self.shown = v;
        self
    }

    /// Update the value (clamped); the fill animates toward it.
    pub fn set(&self, value: f32) {
        self.value.set(value.clamp(0.0, 1.0));
    }

    /// The value signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<f32> {
        self.value
    }
}

impl Component for ProgressBar {
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
        let (surface, accent, glow_c, muted, theme_radius) = {
            let t = cx.theme();
            (t.surface, t.accent, t.glow, t.muted, t.radius)
        };
        let b = self.base.bounds;
        // Follow the theme radius, clamped to the bar's pill max (0 → square).
        let radius = theme_radius.min((b.size.h / 2.0) as f32);

        // Track.
        cx.rect(
            b,
            surface,
            Some(Border {
                color: muted,
                width: 1.0,
            }),
            radius,
            None,
        );

        // Accent fill from the left, eased to the current fraction.
        let frac = self.shown.clamp(0.0, 1.0) as f64;
        if frac > 0.0 {
            let fill = Rectangle::new(b.loc, Size::new(b.size.w * frac, b.size.h));
            let glow = Some(Glow {
                color: glow_c,
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY,
            });
            cx.rect(fill, accent, None, radius, glow);
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let target = self.value.get_untracked().clamp(0.0, 1.0);
        if (self.shown - target).abs() < 1e-3 {
            self.shown = target;
            return false;
        }
        let step = dt / ANIM_DURATION;
        self.shown = if self.shown < target {
            (self.shown + step).min(target)
        } else {
            (self.shown - step).max(target)
        };
        true
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for ProgressBar {}
