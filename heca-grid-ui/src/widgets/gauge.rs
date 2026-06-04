//! [`Gauge`] — a segmented Tron energy meter. A display widget: lit segments
//! grow with a `0.0..=1.0` value and shift color along the bar (success → warning
//! → danger), so a near-full gauge reads "hot". Bind it to a `Signal<f32>` via
//! [`value`](Gauge::value) / [`set`](Gauge::set).

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::Glow;
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Default width (logical px).
const DEFAULT_WIDTH: f32 = 168.0;
/// Height (logical px).
const HEIGHT: f32 = 18.0;
/// Number of segments.
const SEGMENTS: usize = 12;
/// Gap between segments (logical px).
const SEG_GAP: f64 = 3.0;
/// Unlit segment alpha.
const UNLIT_ALPHA: u8 = 40;

/// A segmented energy meter.
pub struct Gauge {
    base: Base,
    value: Signal<f32>,
}

impl Gauge {
    /// A new gauge at 0.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.width = Length::Px(DEFAULT_WIDTH);
        base.style.height = Length::Px(HEIGHT);
        Self {
            base,
            value: signal(0.0),
        }
    }

    /// Set the initial value (clamped to `0.0..=1.0`).
    pub fn value(self, value: f32) -> Self {
        self.value.set(value.clamp(0.0, 1.0));
        self
    }

    /// Update the value (clamped).
    pub fn set(&self, value: f32) {
        self.value.set(value.clamp(0.0, 1.0));
    }

    /// The value signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<f32> {
        self.value
    }
}

impl Component for Gauge {
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
        let (success, warning, danger, muted, glow_c) = {
            let t = cx.theme();
            (t.success, t.warning, t.danger, t.muted, t.glow)
        };
        let b = self.base.bounds;
        let value = self.value.get_untracked().clamp(0.0, 1.0);
        let lit = (value * SEGMENTS as f32).round() as usize;

        let n = SEGMENTS as f64;
        let seg_w = ((b.size.w - (n - 1.0) * SEG_GAP) / n).max(0.0);
        for i in 0..SEGMENTS {
            let x = b.loc.x + i as f64 * (seg_w + SEG_GAP);
            let rect = Rectangle::new(Point::new(x, b.loc.y), Size::new(seg_w, b.size.h));
            let frac = i as f32 / SEGMENTS as f32;
            // Color shifts along the bar: low=success, mid=warning, high=danger.
            let color = if frac < 0.6 {
                success
            } else if frac < 0.85 {
                warning
            } else {
                danger
            };
            if i < lit {
                let glow = Some(Glow {
                    color: glow_c,
                    radius: 8.0,
                    intensity: 0.12,
                });
                cx.rect(rect, color, None, 1.0, glow);
            } else {
                cx.rect(rect, muted.with_alpha(UNLIT_ALPHA), None, 1.0, None);
            }
        }
    }
}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Gauge {}
