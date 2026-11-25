//! [`RailCell`] — a focusable **icon cell** for the collapsed chrome rail.
//!
//! Where [`DockFrame::rail`](super::DockFrame::rail) folds a whole *tool* dock to
//! a single icon, a **list** dock (workspaces / columns / panes) must keep one
//! cell **per item** when collapsed — you still need to see, focus, and pick each
//! pane. `RailCell` is that per-item cell: a themed square holding one
//! [`Icon`], with the same interactive chrome as [`Row`](super::Row)
//! (hover tint, active selection + same-hue border/glow, press flash, focus ring,
//! `on_activate` on click / Enter / Space).
//!
//! Items are **icons by default**. The move/swap/focus-select **pick letters**
//! that appear over a cell are not built in here — wrap the cell in the generic
//! [`KeyHint`](super::KeyHint) overlay so the *same* keycap mechanism is reused
//! across the rail, content-area panes, and command palettes.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component, PaintCx};
use crate::effects::Flash;
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Justify, Length};
use crate::widgets::Icon;

/// Default square cell extent (logical px).
const DEFAULT_CELL: f32 = 40.0;
/// Halo falloff for the resting bare glyph, in logical px. Matches [`Icon`]'s own so
/// a rail cell and a standalone glowing icon read the same.
const ICON_HALO_RADIUS: f32 = 6.0;
// Per-widget glow, intended: this widget's own measure, still scaled by `glow_size`.
// See docs/widgets.md → "Each widget's own glow and border".
/// Glow radius/intensity of the active cell (scaled by the theme's `glow_size`).
const ACTIVE_GLOW_RADIUS: f32 = 9.0;
const ACTIVE_GLOW_INTENSITY: f32 = 0.22;

/// A focusable, selectable square icon cell.
pub struct RailCell {
    base: Base,
    /// The glyph's name — this cell's accessible name. See [`RailCell::new`].
    name: &'static str,
    /// Square cell extent (px).
    cell: f32,
    /// Active (current / selected) state — accent tint + border + glow.
    active: Signal<bool>,
    flash: Flash,
}

#[heca_grid_ui_macros::props]
impl RailCell {
    /// A new cell wrapping `icon`, centered in a square. Make it
    /// clickable/keyboard-activatable with [`on_click`](crate::builders::EventExt::on_click).
    pub fn new(icon: Icon) -> Self {
        // Its name, taken while the icon is still typed — a rail cell IS its glyph. Same reason as
        // `IconButton::new`: without it the cell has no identity at all, and `Icon` must stay silent
        // so it does not shadow a `Label` beside it elsewhere.
        let name = icon.glyph_signal().get_untracked().name();
        let mut base = Base::new();
        // Center the single icon child both ways within the square cell.
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Center;
        base.style.layout.justify = Justify::Center;
        base.children.push(crate::WidgetModel::new(icon));
        let mut cell = Self {
            base,
            name,
            cell: DEFAULT_CELL,
            active: signal(false),
            flash: Flash::new(),
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
    pub fn active(self, active: impl crate::builders::IntoBoolValue) -> Self {
        active.into_bool_value().show_in(self.active);
        self
    }

    /// The active-state signal — bind selection to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.active
    }

    fn interactive(&self) -> bool {
        self.base.activatable
    }
}

impl Component for RailCell {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// **An icon-only control is its glyph, so that is its name.** See [`RailCell::new`].
    fn text_summary(&self) -> Option<String> {
        Some(self.name.to_string())
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
            (
                cx.accent(),
                t.colors.glow,
                t.colors.foreground,
                t.colors.control_radius(),
                t.focus_border_width,
            )
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
                Some(Border {
                    color: accent.with_alpha(cx.theme().colors.interaction.row_active_border),
                    width: sel_border_w,
                }),
                cell_radius,
                Some(Glow {
                    color: glow_c,
                    radius: ACTIVE_GLOW_RADIUS,
                    intensity: ACTIVE_GLOW_INTENSITY,
                }),
            );
        } else if self.base.hovered() {
            cx.rect(
                b,
                foreground.with_alpha(cx.theme().colors.interaction.row_hover_fill),
                None,
                cell_radius,
                None,
            );
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
                crate::component::paint_child(child.as_ref(), cx);
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

    /// Used: the press flash, shown only when this control is the one that used the event.
    fn show_press(&mut self) {
        self.flash.trigger();
    }

    fn is_control(&self) -> bool {
        true
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
