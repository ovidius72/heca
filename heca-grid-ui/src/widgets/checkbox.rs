//! [`Checkbox`] — a boolean change widget: a bordered square that fills with a
//! glowing accent indicator (popping in from the center) when checked. Shares
//! the change-widget pattern with [`Toggle`](super::Toggle): flipping it emits
//! `Action::value("checkbox-change", SignalData::Bool(new))` to an
//! [`on_change`](Checkbox::on_change) handler.
//!
//! An optional [`label`](Checkbox::label) can sit on either side
//! ([`LabelSide`]); clicking anywhere on the box **or** label toggles it. Like
//! the other interactive widgets it reuses [`Flash`], is
//! [`focusable`](Component::focusable), activates on Space/Enter, honors
//! [`Base::disabled`](crate::component::Base), and shows a focus-visible ring.

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, Glow, TextAlign, TextStyle};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Box side length (logical px).
const BOX_SIZE: f64 = 22.0;
/// Checked indicator size as a fraction of the box at full-on.
const INNER_FRAC: f64 = 0.55;
/// Indicator corner radius.
const INNER_RADIUS: f32 = 2.0;
/// Gap between the box and its label.
const LABEL_GAP: f64 = 8.0;
/// Label font size.
const LABEL_FS: f32 = 14.0;
/// Seconds for a full check/uncheck pop.
const ANIM_DURATION: f32 = 0.10;
/// Checked-state glow spread radius (px).
const GLOW_RADIUS: f32 = 14.0;
/// Checked-state glow peak intensity.
const GLOW_INTENSITY: f32 = 0.09;

/// Which side of the box the [`Checkbox`] label sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelSide {
    /// Label to the right of the box (default).
    #[default]
    Right,
    /// Label to the left of the box.
    Left,
}

/// A boolean checkbox with an optional, clickable label. Emits `checkbox-change`
/// with the new [`bool`] when toggled (pointer press on box or label, or
/// Space/Enter while focused).
pub struct Checkbox {
    base: Base,
    /// Checked state, exposed reactively via [`state`](Checkbox::state).
    checked: Signal<bool>,
    label: Option<String>,
    label_side: LabelSide,
    /// Animated indicator amount, 0.0 (empty) → 1.0 (checked).
    progress: f32,
    /// Press flash (brightens on toggle, fades out).
    flash: Flash,
    hovered: Signal<bool>,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Checkbox {
    /// A new checkbox, unchecked and label-less by default.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        base.style.layout.width = Length::Px(BOX_SIZE as f32);
        base.style.layout.height = Length::Px(BOX_SIZE as f32);
        Self {
            base,
            checked: signal(false),
            label: None,
            label_side: LabelSide::Right,
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

    /// Add a label next to the box. Clicking the label toggles the checkbox.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self.remeasure();
        self
    }

    /// Choose which side the label sits on (default [`LabelSide::Right`]).
    pub fn label_side(mut self, side: LabelSide) -> Self {
        self.label_side = side;
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

    /// Box size, label gap and label font scaled by the size variant, so the whole
    /// control (and its hit boxes) grow/shrink together.
    fn box_size(&self) -> f64 {
        BOX_SIZE * self.base.size_scale() as f64
    }
    fn label_gap(&self) -> f64 {
        LABEL_GAP * self.base.size_scale() as f64
    }
    fn label_fs(&self) -> f32 {
        // Label is text → font scale (not the tighter padding scale).
        LABEL_FS * self.base.style.layout.size.font_scale()
    }

    fn remeasure(&mut self) {
        let box_size = self.box_size() as f32;
        match &self.label {
            Some(label) => {
                let fs = self.label_fs();
                let text_w = label.chars().count() as f32 * fs * MONO_ADVANCE_RATIO;
                let line = fs * MONO_LINE_RATIO;
                self.base.style.layout.width = Length::Px(box_size + self.label_gap() as f32 + text_w);
                self.base.style.layout.height = Length::Px(box_size.max(line));
            }
            None => {
                self.base.style.layout.width = Length::Px(box_size);
                self.base.style.layout.height = Length::Px(box_size);
            }
        }
    }

    /// The box rect (vertically centered), positioned per the label side.
    fn box_rect(&self) -> Rectangle {
        let b = self.base.bounds;
        let box_size = self.box_size();
        let y = b.loc.y + (b.size.h - box_size) / 2.0;
        let x = if self.label.is_some() && self.label_side == LabelSide::Left {
            b.loc.x + b.size.w - box_size
        } else {
            b.loc.x
        };
        Rectangle::new(Point::new(x, y), Size::new(box_size, box_size))
    }

    /// The label text rect (renderer centers vertically).
    fn label_rect(&self) -> Rectangle {
        let b = self.base.bounds;
        let x = match self.label_side {
            LabelSide::Left => b.loc.x,
            LabelSide::Right => b.loc.x + self.box_size() + self.label_gap(),
        };
        let w = (b.size.w - self.box_size() - self.label_gap()).max(0.0);
        Rectangle::new(Point::new(x, b.loc.y), Size::new(w, b.size.h))
    }

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
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
}

impl Component for Checkbox {
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
        let disabled = self.base.disabled.get_untracked();
        let (surface, accent, glow_c, muted, foreground, radius, bw, ia) = {
            let t = cx.theme();
            (
                t.colors.surface,
                t.colors.accent,
                t.colors.glow,
                t.colors.muted,
                t.colors.foreground,
                t.colors.control_radius(),
                t.colors.border_width,
                t.colors.interaction,
            )
        };
        let p = self.progress.clamp(0.0, 1.0);
        let bx = self.box_rect();

        // Box: dark fill, border firms muted → accent. Radius + border width from
        // the theme so the global settings scale this proportionally. A faint theme
        // rest glow (`interaction.control_rest_glow`) on the BOX (not the label)
        // gives it the shared neon identity at rest; `glow_size` scales it (T011).
        let rest_border = ia.control_rest_border as f32;
        let border_a = rest_border + (255.0 - rest_border) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: bw,
        };
        let box_glow = if disabled { None } else { cx.rest_glow(GLOW_RADIUS) };
        cx.rect(bx, surface, Some(border), radius, box_glow);

        // Checked indicator: an accent square that pops in from the box center.
        if p > 0.0 {
            let inner = bx.size.w * INNER_FRAC * p as f64;
            let indicator = Rectangle::new(
                Point::new(
                    bx.loc.x + (bx.size.w - inner) / 2.0,
                    bx.loc.y + (bx.size.h - inner) / 2.0,
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

        // Label text.
        if let Some(label) = &self.label {
            cx.text(
                self.label_rect(),
                label,
                foreground,
                self.label_fs(),
                TextAlign::Start,
                TextStyle::REGULAR,
            );
        }

        // Press flash over the box (active widgets only).
        if !disabled {
            cx.flash(bx, self.flash.amount() * 0.6, radius);
        }

        // Dim the whole control (box + label) when disabled.
        if disabled {
            cx.dim(self.base.bounds, 0.0);
        }

        // Focus ring around the BOX only (theme-aware color) — the standard control
        // ring (web input outline / macOS): the label is clickable but not ringed,
        // so keyboard focus doesn't draw a heavy frame around the whole row.
        if !disabled && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(bx, ring, radius);
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

        let target = if self.checked.get_untracked() {
            1.0
        } else {
            0.0
        };
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
        // Damage just our own rect so the check pop-in doesn't force a full redraw.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Checkbox {}
