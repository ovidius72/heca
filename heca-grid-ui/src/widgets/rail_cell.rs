//! [`RailCell`] — a focusable **icon cell** for the collapsed chrome rail.
//!
//! Where [`DockFrame::rail`](super::DockFrame::rail) folds a whole *tool* dock to
//! a single icon, a **list** dock (workspaces / columns / panes) must keep one
//! cell **per item** when collapsed — you still need to see, focus, and pick each
//! pane. `RailCell` is that per-item cell: a themed square holding one
//! [`Icon`](super::Icon), with the same interactive chrome as [`Row`](super::Row)
//! (hover tint, active selection + same-hue border/glow, press flash, focus ring,
//! `on_activate` on click / Enter / Space).
//!
//! Items are **icons by default**. The move/swap/focus-select **pick letters**
//! that appear over a cell are not built in here — wrap the cell in the generic
//! [`KeyHint`](super::KeyHint) overlay so the *same* keycap mechanism is reused
//! across the rail, content-area panes, and command palettes.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Justify, Length};
use crate::widgets::Icon;

/// Default square cell extent (logical px).
const DEFAULT_CELL: f32 = 40.0;
/// Halo falloff for the resting bare glyph, in logical px. Matches [`Icon`]'s own so
/// a rail cell and a standalone glowing icon read the same.
const ICON_HALO_RADIUS: f32 = 6.0;
/// Glow radius/intensity of the active cell (scaled by the theme's `glow_size`).
const ACTIVE_GLOW_RADIUS: f32 = 9.0;
const ACTIVE_GLOW_INTENSITY: f32 = 0.22;

/// A focusable, selectable square icon cell.
pub struct RailCell {
    base: Base,
    /// Square cell extent (px).
    cell: f32,
    /// Active (current / selected) state — accent tint + border + glow.
    active: Signal<bool>,
    flash: Flash,
    on_activate: Option<Box<dyn Fn()>>,
}

#[heca_grid_ui_macros::props]
impl RailCell {
    /// A new cell wrapping `icon`, centered in a square. Make it
    /// clickable/keyboard-activatable with [`on_activate`](RailCell::on_activate).
    pub fn new(icon: Icon) -> Self {
        let mut base = Base::new();
        // Center the single icon child both ways within the square cell.
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Center;
        base.style.layout.justify = Justify::Center;
        base.children.push(Box::new(icon));
        let mut cell = Self {
            base,
            cell: DEFAULT_CELL,
            active: signal(false),
            flash: Flash::new(),
            on_activate: None,
        };
        cell.remeasure();
        cell
    }

    /// Square cell extent in logical px (default 40).
    #[heca_grid_ui_macros::prop]
    pub fn cell_size(mut self, px: f32) -> Self {
        self.cell = px;
        self.remeasure();
        self
    }

    /// Set the active (selected/current) state.
    #[heca_grid_ui_macros::prop]
    pub fn active(self, active: bool) -> Self {
        self.active.set(active);
        self
    }

    /// The active-state signal — bind selection to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.active
    }

    /// Make the cell clickable/keyboard-activatable (also makes it focusable). The
    /// host maps activation to its intent (focus the pane, pick the swap target…).
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_activate(mut self, f: impl Fn() + 'static) -> Self {
        self.on_activate = Some(Box::new(f));
        self.base.focusable = true; // interactive cells are focusable (Component::focusable)
        self.base.one_click_target = true; // and one click target (Base::one_click_target)
        self
    }

    fn interactive(&self) -> bool {
        self.on_activate.is_some()
    }

    fn activate(&mut self) {
        self.flash.trigger();
        if let Some(f) = &self.on_activate {
            f();
        }
    }
}

impl Component for RailCell {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// A fixed square along both axes.
    fn remeasure(&mut self) {
        self.base.style.layout.width = Length::Px(self.cell);
        self.base.style.layout.height = Length::Px(self.cell);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let active = self.active.get_untracked();
        let (accent, glow_c, foreground, ctrl_radius, sel_border_w) = {
            let t = cx.theme();
            (t.colors.accent, t.colors.glow, t.colors.foreground, t.colors.control_radius(), t.focus_border_width)
        };
        let b = self.base.bounds;

        // Optional persistent background under the interactive overlay.
        cx.paint_base(&self.base);

        // Cell chrome — themed radius/border/glow, never literals. Rest = bare
        // icon; hover = faint accent wash; active = accent tint + crisp same-hue
        // border + glow (the "selected pane" cue, readable on dark).
        let cell_radius = ctrl_radius.min((b.size.h / 2.0) as f32);
        if active {
            cx.rect(
                b,
                accent.with_alpha(cx.theme().colors.interaction.row_active_fill),
                Some(Border { color: accent.with_alpha(cx.theme().colors.interaction.row_active_border), width: sel_border_w }),
                cell_radius,
                Some(Glow { color: glow_c, radius: ACTIVE_GLOW_RADIUS, intensity: ACTIVE_GLOW_INTENSITY }),
            );
        } else if self.base.hovered() {
            cx.rect(b, foreground.with_alpha(cx.theme().colors.interaction.row_hover_fill), None, cell_radius, None);
        }

        // The icon (carries its own status color). At rest the cell draws no surface
        // at all — the bare glyph *is* the resting look — so there is nothing to carry
        // the halo that every bordered surface gets. Publish one for the subtree and
        // let the glyph pull it; the cell cannot style a child it holds as
        // `impl Component`. Hover and active already have their own lit chrome, so
        // they do not double it.
        let rest_glow = (!active && !self.base.hovered() && !disabled)
            .then(|| cx.rest_glow(ICON_HALO_RADIUS))
            .flatten();
        cx.with_content_glow(rest_glow, |cx| {
            for child in &self.base.children {
                child.paint(cx);
            }
        });

        if self.interactive() && !disabled {
            cx.flash(b, self.flash.amount() * 0.5, cell_radius);
        }
        if disabled {
            cx.dim(b, cell_radius);
        }
        if self.interactive()
            && !disabled
            && self.base.shows_focus_ring()
            && cx.theme().colors.show_focus_border
        {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(b, ring, cell_radius);
        }
    }

    /// Capture, not bubble: this control is **one click target and one Tab stop**
    /// (`Base::focus_barrier`), so its composed content — an `Icon`, a `Label`, anything — must
    /// never see the press first. Handling it before the children is what keeps that true.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.interactive() || self.base.disabled.get_untracked() {
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
    /// [`EventExt`](crate::builders::EventExt) handler registered on this widget gets first
    /// refusal and can take the click with `stop_propagation`.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if !self.interactive() || self.base.disabled.get_untracked() {
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
        let mut animating = self.flash.tick(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        // Damage our own rect (which contains the icon) so a press flash doesn't
        // force a full redraw.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for RailCell {}
impl StyleExt for RailCell {}
impl Parent for RailCell {}
