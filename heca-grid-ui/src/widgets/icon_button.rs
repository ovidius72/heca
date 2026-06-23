//! [`IconButton`] — a compact, clickable **icon affordance** (toolbar / header
//! button). It is the icon-only cousin of [`Button`](super::Button): a single
//! [`Icon`](super::Icon) with the same interactive chrome — an animated hover
//! tint + border (+ optional glow), a press [`Flash`], a keyboard focus ring, and
//! `on_click` (mouse + Enter/Space). At rest it is just the icon (ghost); the
//! tinted background fades in on hover, so a row of them stays quiet until used.
//!
//! Hugging its icon by default, it sizes to the glyph + padding; pin a square with
//! [`size`](IconButton::size). The hover/press hue is theme-driven (the accent,
//! or an override via [`tone`](IconButton::tone)), never baked in.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Justify, Length};
use crate::widgets::Icon;

/// Inset of the icon from the button edge (logical px) when not pinned to a size.
const DEFAULT_PAD: f32 = 8.0;
/// Seconds for a full hover transition.
const HOVER_DURATION: f32 = 0.10;
/// Border width of the hover frame.
const BORDER_W: f32 = 1.3;
/// Peak alpha of the hover fill (tone-tinted) and border.
const HOVER_FILL_ALPHA: f32 = 28.0;
const HOVER_BORDER_ALPHA: f32 = 190.0;
/// Held-on ("active"/toggled) fill + border alpha — a persistent tone wash with a
/// firm border, mirroring the [`Toggle`](super::Toggle) on-state. Stronger than the
/// hover fill so an engaged button reads as a *status*, not a transient hover.
const ACTIVE_FILL_ALPHA: f32 = 64.0;
const ACTIVE_BORDER_ALPHA: f32 = 215.0;
/// Hover glow spread + peak intensity (scaled by the theme `glow_size` + hover).
const GLOW_RADIUS: f32 = 10.0;
const GLOW_INTENSITY: f32 = 0.18;

/// A compact, clickable icon button.
pub struct IconButton {
    base: Base,
    /// Explicit square size (px); otherwise hugs the icon + padding.
    cell: Option<f32>,
    /// Hover/press hue (default: theme accent).
    tone: Option<Color>,
    /// Held-on visual: a persistent tone-tinted frame marking the button as a
    /// toggled-on status (e.g. a zoomed column / floating pane). Independent of hover.
    active: bool,
    show_glow: bool,
    /// Animated hover amount, 0.0 (rest) → 1.0 (hovered).
    progress: f32,
    flash: Flash,
    hovered: Signal<bool>,
    on_click: Option<Box<dyn Fn()>>,
}

impl IconButton {
    /// A new icon button wrapping `icon`, centered.
    pub fn new(icon: Icon) -> Self {
        let mut base = Base::new();
        // Center the single icon child; pad it so the hover frame has breathing room.
        base.style.direction = Direction::Row;
        base.style.align = Align::Center;
        base.style.justify = Justify::Center;
        base.style.padding = DEFAULT_PAD;
        base.children.push(Box::new(icon));
        Self {
            base,
            cell: None,
            tone: None,
            active: false,
            show_glow: true,
            progress: 0.0,
            flash: Flash::new(),
            hovered: signal(false),
            on_click: None,
        }
    }

    /// Pin a square button of `px` (icon centered); otherwise it hugs the icon.
    /// Named `cell` (not `size`) so the shared [`LayoutExt::size`] size-variant
    /// builder stays available on `IconButton`.
    pub fn cell(mut self, px: f32) -> Self {
        self.cell = Some(px);
        self.remeasure();
        self
    }

    /// Override the hover/press hue (default: theme accent).
    pub fn tone(mut self, c: Color) -> Self {
        self.tone = Some(c);
        self
    }

    /// Enable or disable the hover glow (default: enabled).
    pub fn glow(mut self, enabled: bool) -> Self {
        self.show_glow = enabled;
        self
    }

    /// Mark the button as **held on** (toggled). When `true` it paints a persistent
    /// tone-tinted fill + firm border (the held version of its hover frame, matching
    /// the [`Toggle`](super::Toggle) on-state) so it reads as an active *status*
    /// rather than a passive icon. Hover/press still layer on top.
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    /// Set the click callback (also makes it focusable).
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// The hover-state signal.
    pub fn hovered(&self) -> Signal<bool> {
        self.hovered
    }

    fn activate(&mut self) {
        self.flash.trigger();
        if let Some(f) = &self.on_click {
            f();
        }
    }
}

impl Component for IconButton {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        self.on_click.is_some() && !self.base.disabled.get_untracked()
    }

    /// A pinned square, or auto (hug the icon + padding) when unset. The size
    /// variant scales the padding and cascades to the icon child so the whole
    /// affordance grows/shrinks together.
    fn remeasure(&mut self) {
        let size = self.base.style.size;
        if let Some(icon) = self.base.children.first_mut() {
            icon.base_mut().style.size = size;
        }
        self.base.style.padding = DEFAULT_PAD * size.pad_scale();
        let len = self.cell.map(Length::Px).unwrap_or(Length::Auto);
        self.base.style.width = len;
        self.base.style.height = len;
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let (accent, glow_c, ctrl_radius) = {
            let t = cx.theme();
            (t.accent, t.glow, t.control_radius())
        };
        let tone = self.tone.unwrap_or(accent);
        let p = self.progress.clamp(0.0, 1.0);
        let b = self.base.bounds;
        let radius = ctrl_radius.min((b.size.h / 2.0) as f32);

        // Tone-tinted frame: a persistent wash when `active` (held-on status), the
        // hover frame fading in by `p`, whichever is stronger. Hover layers on top of
        // active so an engaged button still brightens under the cursor.
        let (active_fill, active_border) = if self.active {
            (ACTIVE_FILL_ALPHA, ACTIVE_BORDER_ALPHA)
        } else {
            (0.0, 0.0)
        };
        let fill_a = (HOVER_FILL_ALPHA * p).max(active_fill);
        let border_a = (HOVER_BORDER_ALPHA * p).max(active_border);
        if fill_a > 0.0 || border_a > 0.0 {
            // Glow holds steady while active, otherwise tracks the hover amount.
            let glow_amt = if self.active { 1.0 } else { p };
            let g = (self.show_glow).then_some(Glow {
                color: glow_c,
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY * glow_amt,
            });
            cx.rect(
                b,
                tone.with_alpha(fill_a as u8),
                Some(Border {
                    color: tone.with_alpha(border_a as u8),
                    width: BORDER_W,
                }),
                radius,
                g,
            );
        }

        // The icon itself.
        for child in &self.base.children {
            child.paint(cx);
        }

        if !disabled {
            cx.flash(b, self.flash.amount() * 0.5, radius);
        }
        if disabled {
            cx.dim(b, radius);
        }
        if self.focusable()
            && self.base.focus_visible.get_untracked()
            && cx.theme().show_focus_border
        {
            cx.corner_brackets(b, accent);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if self.on_click.is_none() || self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerMoved { pos } => {
                let inside = self.base.bounds.contains(*pos);
                if self.hovered.get_untracked() != inside {
                    self.hovered.set(inside);
                }
                Handled::No
            }
            Event::PointerPressed { pos } if self.base.bounds.contains(*pos) => {
                self.activate();
                Handled::Yes
            }
            Event::Key {
                key: GridKey::Enter | GridKey::Space,
                pressed: true,
            } => {
                self.activate();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        let target = if self.hovered.get_untracked() {
            1.0
        } else {
            0.0
        };
        if (self.progress - target).abs() >= 1e-3 {
            let step = dt / HOVER_DURATION;
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
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        // Damage our own rect (which contains the icon) so the hover/press animation
        // doesn't force a full redraw.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for IconButton {}
