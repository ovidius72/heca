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
use crate::reactive::{Signal, SignalGet};
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Justify, Length};
use crate::widgets::Icon;

/// Inset of the icon from the button edge (logical px) when not pinned to a size.
const DEFAULT_PAD: f32 = 8.0;
/// Seconds for a full hover transition.
const HOVER_DURATION: f32 = 0.10;
/// Hover glow spread + peak intensity (scaled by the theme `glow_size` + hover).
const GLOW_RADIUS: f32 = 10.0;
const GLOW_INTENSITY: f32 = 0.18;

/// A compact, clickable icon button.
pub struct IconButton {
    base: Base,
    /// The glyph's name — this control's accessible name. See [`IconButton::new`].
    name: &'static str,
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
    on_click: Option<Box<dyn Fn()>>,
}

#[heca_grid_ui_macros::props]
impl IconButton {
    /// A new icon button wrapping `icon`, centered.
    pub fn new(icon: Icon) -> Self {
        // **Its name, taken while the icon is still typed.** An icon-only control IS its glyph, so
        // this is what it is called — and without it `nav::identity_of` returns `None` and the
        // button has no identity at all: nothing about it can be remembered, not a hint letter, not
        // a cursor position (F003/P082/T444). Only `Label`, `Badge`, `BadgeButton` and `Tag`
        // supplied a name before, so every pane-header action, close button and rail cell was in
        // that state. Read here rather than from the child, because `Icon` must stay silent: the
        // accessible-name walk takes the first child that answers, so an `Icon` naming itself would
        // shadow the `Label` beside it in a `Choice` and `Select` would report "warning" instead of
        // "HIGH".
        let name = icon.glyph_signal().get_untracked().name();
        let mut base = Base::new();
        // Center the single icon child; pad it so the hover frame has breathing room.
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Center;
        base.style.layout.justify = Justify::Center;
        base.style.layout.padding = DEFAULT_PAD;
        base.children.push(Box::new(icon));
        Self {
            base,
            name,
            cell: None,
            tone: None,
            active: false,
            show_glow: true,
            progress: 0.0,
            flash: Flash::new(),
            on_click: None,
        }
    }

    /// Pin a square button of `px` (icon centered); otherwise it hugs the icon.
    /// Named `cell` (not `size`) so the shared [`LayoutExt::size`] size-variant
    /// builder stays available on `IconButton`.
    #[heca_grid_ui_macros::prop]
    pub fn cell(mut self, px: f32) -> Self {
        self.cell = Some(px);
        self.remeasure();
        self
    }

    /// Override the hover/press hue (default: theme accent).
    #[heca_grid_ui_macros::prop]
    pub fn tone(mut self, c: Color) -> Self {
        self.tone = Some(c);
        self
    }

    /// Enable or disable the hover glow (default: enabled).
    #[heca_grid_ui_macros::prop]
    pub fn glow(mut self, enabled: bool) -> Self {
        self.show_glow = enabled;
        self
    }

    /// Mark the button as **held on** (toggled). When `true` it paints a persistent
    /// tone-tinted fill + firm border (the held version of its hover frame, matching
    /// the [`Toggle`](super::Toggle) on-state) so it reads as an active *status*
    /// rather than a passive icon. Hover/press still layer on top.
    #[heca_grid_ui_macros::prop]
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    /// Set the click callback (also makes it focusable).
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self.base.activatable = true; // and pickable — a letter runs this (Base::activatable)
        self.base.focusable = true; // clickable icon buttons are focusable (Component::focusable)
        self.base.one_click_target = true; // and one click target (Base::one_click_target)
        self
    }

    /// The hover-state signal.
    pub fn hovered(&self) -> Signal<bool> {
        self.base.pointer.hovered
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

    /// **An icon-only control is its glyph, so that is its name.** See [`IconButton::new`] for why
    /// it is read there and not delegated to the `Icon` child.
    fn text_summary(&self) -> Option<String> {
        Some(self.name.to_string())
    }

    /// A pinned square, or auto (hug the icon + padding) when unset. The size
    /// variant scales the padding; the **icon child inherits the variant from the layout pass**
    /// (see [`Style::size_explicit`](crate::style::Style::size_explicit)), so the whole affordance
    /// grows/shrinks together without this widget copying the variant into its child.
    fn remeasure(&mut self) {
        let size = self.base.style.layout.size;
        self.base.style.layout.padding = DEFAULT_PAD * size.pad_scale();
        let len = self.cell.map(Length::Px).unwrap_or(Length::Auto);
        self.base.style.layout.width = len;
        self.base.style.layout.height = len;
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let (accent, glow_c, ctrl_radius, border_width, focus_border_width, ia) = {
            let t = cx.theme();
            (t.colors.accent, t.colors.glow, t.colors.control_radius(), t.colors.border_width, t.focus_border_width, t.colors.interaction)
        };
        // **Own tone → the tone a container published → the theme accent** — the same chain a
        // `Label`'s ink follows, applied to chrome. `None` unless a container asked (nothing does
        // by default), so a button on its own looks exactly as it did (F003/P096/T484).
        let tone = self.tone.or_else(|| cx.control_tone()).unwrap_or(accent);
        let p = self.progress.clamp(0.0, 1.0);
        let b = self.base.bounds;
        let radius = ctrl_radius.min((b.size.h / 2.0) as f32);

        // Tone-tinted frame: a persistent wash when `active` (held-on status), the
        // hover frame fading in by `p`, whichever is stronger. Hover layers on top of
        // active so an engaged button still brightens under the cursor.
        let (active_fill, active_border) = if self.active {
            (ia.control_active_fill as f32, ia.control_active_border as f32)
        } else {
            (0.0, 0.0)
        };
        let fill_a = (ia.control_hover_fill as f32 * p).max(active_fill);
        let border_a = (ia.control_hover_border as f32 * p).max(active_border);
        if fill_a > 0.0 || border_a > 0.0 {
            // Glow holds steady while active, otherwise tracks the hover amount.
            let glow_amt = if self.active { 1.0 } else { p };
            let g = (self.show_glow).then_some(Glow {
                color: glow_c,
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY * glow_amt,
            });
            // Frame line width: when `active` (a held-on *status*, i.e. a selection
            // cue) use `focus_border_width` so it stays visible even with decorative
            // borders off; a transient hover frame follows the global `border_width`
            // and vanishes at 0. The tone-tinted fill stays either way.
            let line_w = if self.active { focus_border_width } else { border_width };
            let frame_border = (line_w > 0.0)
                .then_some(Border { color: tone.with_alpha(border_a as u8), width: line_w });
            cx.rect(b, tone.with_alpha(fill_a as u8), frame_border, radius, g);
        }

        // The icon itself.
        for child in &self.base.children {
            crate::component::paint_child(child.as_ref(), cx);
        }

        if !disabled {
            cx.flash(b, self.flash.amount() * 0.5, radius);
        }
        if disabled {
            cx.dim(b, radius);
        }
        if self.focusable() && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(b, ring, radius);
        }
    }

    /// Capture, not bubble: this control is **one click target and one Tab stop**
    /// (`Base::focus_barrier`), so its composed content — an `Icon`, a `Label`, anything — must
    /// never see the press first. Handling it before the children is what keeps that true.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if self.on_click.is_none() || self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::Key { key: GridKey::Enter | GridKey::Space, pressed: true } => {
                self.activate();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// **The click, after its children have declined it.**
    ///
    /// The press is taken in capture (so composed content can never take it first) and the click
    /// it turns into is delivered to whoever took that press — this control — which is what makes
    /// "one control, one click target" a framework rule rather than something each control
    /// arranges by swallowing events. Bubble, not capture, so an
    /// [`ComponentExt`](crate::builders::ComponentExt) handler registered on this widget gets first
    /// refusal and can take the click with `stop_propagation`.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if self.on_click.is_none() || self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::Click(_) => {
                self.activate();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        let target = if self.base.hovered() { 1.0 } else { 0.0 };
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
