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
/// Active-cell fill alpha (accent tint under the icon).
const ACTIVE_FILL_ALPHA: u8 = 34;
/// Hover-cell fill alpha.
const HOVER_FILL_ALPHA: u8 = 18;
/// Alpha of the crisp accent border around the active cell — the main "selected"
/// cue, distinct from a merely tinted/hovered cell.
const ACTIVE_BORDER_ALPHA: u8 = 190;
/// Width of the active cell's border (logical px).
const ACTIVE_BORDER_W: f32 = 1.3;
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
    hovered: Signal<bool>,
    flash: Flash,
    on_activate: Option<Box<dyn Fn()>>,
}

impl RailCell {
    /// A new cell wrapping `icon`, centered in a square. Make it
    /// clickable/keyboard-activatable with [`on_activate`](RailCell::on_activate).
    pub fn new(icon: Icon) -> Self {
        let mut base = Base::new();
        // Center the single icon child both ways within the square cell.
        base.style.direction = Direction::Row;
        base.style.align = Align::Center;
        base.style.justify = Justify::Center;
        base.children.push(Box::new(icon));
        let mut cell = Self {
            base,
            cell: DEFAULT_CELL,
            active: signal(false),
            hovered: signal(false),
            flash: Flash::new(),
            on_activate: None,
        };
        cell.remeasure();
        cell
    }

    /// Square cell extent in logical px (default 40).
    pub fn cell_size(mut self, px: f32) -> Self {
        self.cell = px;
        self.remeasure();
        self
    }

    /// Set the active (selected/current) state.
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
    pub fn on_activate(mut self, f: impl Fn() + 'static) -> Self {
        self.on_activate = Some(Box::new(f));
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

    fn focusable(&self) -> bool {
        self.interactive() && !self.base.disabled.get_untracked()
    }

    /// A fixed square along both axes.
    fn remeasure(&mut self) {
        self.base.style.width = Length::Px(self.cell);
        self.base.style.height = Length::Px(self.cell);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let active = self.active.get_untracked();
        let (accent, glow_c, foreground, ctrl_radius) = {
            let t = cx.theme();
            (t.accent, t.glow, t.foreground, t.control_radius())
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
                accent.with_alpha(ACTIVE_FILL_ALPHA),
                Some(Border { color: accent.with_alpha(ACTIVE_BORDER_ALPHA), width: ACTIVE_BORDER_W }),
                cell_radius,
                Some(Glow { color: glow_c, radius: ACTIVE_GLOW_RADIUS, intensity: ACTIVE_GLOW_INTENSITY }),
            );
        } else if self.hovered.get_untracked() {
            cx.rect(b, foreground.with_alpha(HOVER_FILL_ALPHA), None, cell_radius, None);
        }

        // The icon (carries its own status color).
        for child in &self.base.children {
            child.paint(cx);
        }

        if self.interactive() && !disabled {
            cx.flash(b, self.flash.amount() * 0.5, cell_radius);
        }
        if disabled {
            cx.dim(b, cell_radius);
        }
        if self.interactive()
            && !disabled
            && self.base.focus_visible.get_untracked()
            && cx.theme().show_focus_border
        {
            cx.corner_brackets(b, accent);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if !self.interactive() || self.base.disabled.get_untracked() {
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
            Event::Key { key: GridKey::Enter | GridKey::Space, pressed: true } => {
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
        animating
    }
}

impl LayoutExt for RailCell {}
impl StyleExt for RailCell {}
impl Parent for RailCell {}
