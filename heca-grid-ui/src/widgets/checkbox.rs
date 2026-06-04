//! [`Checkbox`] — a boolean change widget: a bordered square that fills with a
//! glowing accent indicator (popping in from the center) when checked. Shares
//! the change-widget pattern with [`Toggle`](super::Toggle): flipping it emits
//! `Action::value("checkbox-change", SignalData::Bool(new))` to an
//! [`on_change`](Checkbox::on_change) handler.
//!
//! Like the other interactive widgets it reuses [`Flash`], is
//! [`focusable`](Component::focusable), activates on Space/Enter, honors
//! [`Base::disabled`](crate::component::Base), and shows a focus-visible ring.
//! The indicator scales in over time via [`Component::tick`].

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Box side length (logical px).
const BOX_SIZE: f64 = 22.0;
/// Box corner radius (a softened square, distinct from a round radio).
const BOX_RADIUS: f32 = 4.0;
/// Checked indicator size as a fraction of the box at full-on.
const INNER_FRAC: f64 = 0.55;
/// Indicator corner radius.
const INNER_RADIUS: f32 = 2.0;
/// Seconds for a full check/uncheck pop.
const ANIM_DURATION: f32 = 0.10;
/// Checked-state glow spread radius (px).
const GLOW_RADIUS: f32 = 14.0;
/// Checked-state glow peak intensity.
const GLOW_INTENSITY: f32 = 0.09;
/// Border alpha at rest; firms to solid as the box is checked.
const REST_BORDER_ALPHA: f32 = 150.0;

/// A boolean checkbox. Emits `checkbox-change` with the new [`bool`] when toggled
/// (pointer press or Space/Enter while focused).
pub struct Checkbox {
    base: Base,
    /// Checked state, exposed reactively via [`state`](Checkbox::state).
    checked: Signal<bool>,
    /// Animated indicator amount, 0.0 (empty) → 1.0 (checked).
    progress: f32,
    /// Press flash (brightens on toggle, fades out).
    flash: Flash,
    hovered: Signal<bool>,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Checkbox {
    /// A new checkbox, unchecked by default.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.width = Length::Px(BOX_SIZE as f32);
        base.style.height = Length::Px(BOX_SIZE as f32);
        Self {
            base,
            checked: signal(false),
            progress: 0.0,
            flash: Flash::new(),
            hovered: signal(false),
            on_change: None,
        }
    }

    /// Set the initial checked state (no animation).
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked.set(checked);
        self.progress = if checked { 1.0 } else { 0.0 };
        self
    }

    /// Set the change handler. Receives `Action::value("checkbox-change",
    /// SignalData::Bool(new_state))` each time the box is toggled.
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// The checked-state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.checked
    }

    /// Current checked state (untracked read).
    pub fn is_checked(&self) -> bool {
        self.checked.get_untracked()
    }

    /// Flip the state: animate the indicator, flash, and emit `checkbox-change`.
    fn flip(&mut self) {
        let new = !self.checked.get_untracked();
        self.checked.set(new);
        self.flash.trigger();
        if let Some(f) = &self.on_change {
            f(Action::value("checkbox-change", SignalData::Bool(new)));
        }
    }

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
    }
}

impl Component for Checkbox {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        !self.base.disabled.get_untracked()
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let (surface, accent, glow_c, muted) = {
            let t = cx.theme();
            (t.surface, t.accent, t.glow, t.muted)
        };
        let p = self.progress.clamp(0.0, 1.0);
        let b = self.base.bounds;

        // Box: dark fill, border firms muted → accent.
        let border_a = REST_BORDER_ALPHA + (255.0 - REST_BORDER_ALPHA) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: 1.5,
        };
        cx.rect(b, surface, Some(border), BOX_RADIUS, None);

        // Checked indicator: an accent square that pops in from the center,
        // glowing as it lands.
        if p > 0.0 {
            let full = b.size.w.min(b.size.h);
            let inner = full * INNER_FRAC * p as f64;
            let indicator = Rectangle::new(
                Point::new(
                    b.loc.x + (b.size.w - inner) / 2.0,
                    b.loc.y + (b.size.h - inner) / 2.0,
                ),
                Size::new(inner, inner),
            );
            let glow = (!disabled).then_some(Glow {
                color: glow_c,
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY * p,
            });
            cx.rect(indicator, accent, None, INNER_RADIUS, glow);
        }

        // Press flash over the box (active widgets only).
        if !disabled {
            cx.flash(b, self.flash.amount() * 0.6, BOX_RADIUS);
        }

        // Dim the whole control when disabled.
        if disabled {
            cx.dim(b, BOX_RADIUS);
        }

        // Focus-visible ring (keyboard focus only).
        if !disabled && self.base.focus_visible.get_untracked() && cx.theme().show_focus_border {
            cx.corner_brackets(b, accent);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerMoved { pos } => {
                let inside = self.contains(*pos);
                if self.hovered.get_untracked() != inside {
                    self.hovered.set(inside);
                }
                Handled::No
            }
            Event::PointerPressed { pos } if self.contains(*pos) => {
                self.flip();
                Handled::Yes
            }
            Event::Key {
                key: GridKey::Enter | GridKey::Space,
                pressed: true,
            } => {
                self.flip();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;

        let target = if self.checked.get_untracked() { 1.0 } else { 0.0 };
        if (self.progress - target).abs() >= 1e-3 {
            let step = dt / ANIM_DURATION;
            self.progress = if self.progress < target {
                (self.progress + step).min(target)
            } else {
                (self.progress - step).max(target)
            };
            animating = true;
        } else {
            self.progress = target;
        }

        animating |= self.flash.tick(dt);
        animating
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Checkbox {}
