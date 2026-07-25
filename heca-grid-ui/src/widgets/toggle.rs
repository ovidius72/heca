//! [`Toggle`] — a Tron switch: a rounded track with a knob that **slides**
//! left (off) → right (on). It is the first **change widget**: flipping it emits
//! a semantic [`Action::value("toggle-change", …)`](Action::value) carrying the
//! new boolean state to an [`on_change`](Toggle::on_change) handler — the same
//! "app receives only semantic actions" model the catalog prescribes.
//!
//! Like [`Button`](super::Button) it is [`focusable`](Component::focusable),
//! activates on Space/Enter, shows a focus-visible ring, and reuses the shared
//! [`Flash`] press effect. The knob position animates over time via
//! [`Component::tick`].

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::Glow;
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Pill widgets round harder than boxes: the theme radius is multiplied by this
/// (then clamped to the capsule max), so a moderate radius reads as a capsule.
const PILL_RADIUS_MUL: f32 = 2.0;

/// Track width (logical px).
const TRACK_W: f64 = 44.0;
/// Track height (logical px); also drives the pill radius.
const TRACK_H: f64 = 24.0;
/// Gap between the knob and the track edge.
const KNOB_PAD: f64 = 3.0;
/// Seconds for a full off↔on slide.
const ANIM_DURATION: f32 = 0.12;
/// On-state glow spread radius (px).
const GLOW_RADIUS: f32 = 16.0;
/// On-state glow peak intensity.
const GLOW_INTENSITY: f32 = 0.09;

/// A sliding on/off switch. Emits `toggle-change` with the new [`bool`] when
/// flipped (pointer press or Space/Enter while focused).
pub struct Toggle {
    base: Base,
    /// On/off state, exposed reactively via [`state`](Toggle::state).
    on: Signal<bool>,
    /// Animated knob position, 0.0 (off) → 1.0 (on).
    progress: f32,
    /// Press flash (brightens on flip, fades out).
    flash: Flash,
    hovered: Signal<bool>,
    on_change: Option<Box<dyn Fn(Action)>>,
}

#[heca_grid_ui_macros::props]
impl Toggle {
    /// A new toggle, off by default.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        base.style.layout.width = Length::Px(TRACK_W as f32);
        base.style.layout.height = Length::Px(TRACK_H as f32);
        Self {
            base,
            on: signal(false),
            progress: 0.0,
            flash: Flash::new(),
            hovered: signal(false),
            on_change: None,
        }
    }

    /// Set the initial on-state (starts the knob at that end, no animation).
    #[heca_grid_ui_macros::prop]
    pub fn on(mut self, on: bool) -> Self {
        self.on.set(on);
        self.progress = if on { 1.0 } else { 0.0 };
        self
    }

    /// Set the change handler. Receives `Action::value("toggle-change",
    /// SignalData::Bool(new_state))` each time the toggle flips.
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// The on/off state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.on
    }

    /// Current on/off state (untracked read).
    pub fn is_on(&self) -> bool {
        self.on.get_untracked()
    }

    /// Flip the state: animate the knob, flash, and emit `toggle-change`.
    fn flip(&mut self) {
        let new = !self.on.get_untracked();
        self.on.set(new);
        self.flash.trigger();
        if let Some(f) = &self.on_change {
            f(Action::value("toggle-change", SignalData::Bool(new)));
        }
    }

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
    }
}

impl Component for Toggle {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The switch is fixed-size (no text); scale the track by the size variant.
    fn remeasure(&mut self) {
        let s = self.base.size_scale();
        self.base.style.layout.width = Length::Px(TRACK_W as f32 * s);
        self.base.style.layout.height = Length::Px(TRACK_H as f32 * s);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let (surface, accent, glow_c, muted, foreground, theme_radius, ia) = {
            let t = cx.theme();
            (t.colors.surface, t.colors.accent, t.colors.glow, t.colors.muted, t.colors.foreground, t.colors.border_radius, t.colors.interaction)
        };
        let p = self.progress.clamp(0.0, 1.0);
        let track = self.base.bounds;

        // Track: dark (off) → translucent accent wash (on). The border firms
        // muted → solid accent (active border stays 100% opaque, unlike the
        // fill); a glow rises as it turns on.
        let rest_border = ia.control_rest_border as f32;
        let border_a = rest_border + (255.0 - rest_border) * p;
        let border = cx.border(muted.lerp(accent, p).with_alpha(border_a.round() as u8));
        // Rest → on: the theme rest glow (`PaintCx::rest_glow`) carries a faint halo
        // while off, blending into the stronger on-glow as `p` rises — so `glow_size`
        // visibly scales the control at rest too (T011).
        let rest_i = cx.rest_glow(GLOW_RADIUS).map_or(0.0, |g| g.intensity);
        let track_i = (GLOW_INTENSITY * p).max(rest_i);
        let track_glow = (!disabled && track_i > 0.0).then_some(Glow {
            color: glow_c,
            radius: GLOW_RADIUS,
            intensity: track_i,
        });
        // Track radius follows the theme but rounds harder (pill widget), clamped
        // to the pill max — so at a moderate theme radius it reads as a capsule.
        let radius = (theme_radius * PILL_RADIUS_MUL).min((track.size.h / 2.0) as f32);
        let fill = surface.lerp(accent.with_alpha(ia.toggle_on_fill), p);
        cx.rect(track, fill, border, radius, track_glow);

        // Knob: muted gray (off) → light (on) so it reads against the accent
        // fill; slides across the track and glows on. Derived from the *actual*
        // (size-scaled) track rect so it tracks the size variant.
        let knob_pad = KNOB_PAD * self.base.size_scale() as f64;
        let knob_d = track.size.h - 2.0 * knob_pad;
        let travel = track.size.w - 2.0 * knob_pad - knob_d;
        let knob = Rectangle::new(
            Point::new(
                track.loc.x + knob_pad + travel * p as f64,
                track.loc.y + knob_pad,
            ),
            Size::new(knob_d, knob_d),
        );
        let knob_glow = (!disabled && p > 0.0).then_some(Glow {
            color: glow_c,
            radius: GLOW_RADIUS * 0.6,
            intensity: GLOW_INTENSITY * 1.5 * p,
        });
        cx.rect(
            knob,
            muted.lerp(foreground, p),
            None,
            (theme_radius * PILL_RADIUS_MUL).min((knob_d / 2.0) as f32),
            knob_glow,
        );

        // Press flash over the track (active widgets only) — rounded to the pill.
        if !disabled {
            cx.flash(track, self.flash.amount() * 0.6, radius);
        }

        // Dim the whole control when disabled.
        if disabled {
            cx.dim(track, radius);
        }

        // Focus ring — shown whenever focused (theme-aware color, outside the track).
        if !disabled && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(track, ring, radius);
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

        let target = if self.on.get_untracked() { 1.0 } else { 0.0 };
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
        // Damage just our own rect so the knob slide doesn't force a full redraw.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl Default for Toggle {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Toggle {}
