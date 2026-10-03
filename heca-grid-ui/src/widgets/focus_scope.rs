//! [`FocusScope`] — a **keyboard focus scope** for a whole component subtree: it outlines itself
//! while the keyboard is anywhere inside it.
//!
//! Every focusable *control* in the library already draws its own ring
//! ([`Button`](super::Button), [`Item`](super::Item), [`IconButton`](super::IconButton) …): they own
//! their focus, so they own the affordance. What has no owner is the case where the thing that holds
//! keyboard focus is **a whole area** rather than a control — a sidebar container that the scroll
//! keys act on, a panel a mode is aimed at. The focus is not any single widget's: it belongs to the
//! subtree.
//!
//! So `FocusScope` is a **transparent wrapper** around one child that asks the tree one question,
//! the one CSS calls `:focus-within` ([`contains_keyboard`](crate::contains_keyboard)): *is the
//! keyboard anywhere in me?* While it is, it draws the theme's focus ring around the child's bounds —
//! [`PaintCx::focus_ring`], the `focus_ring` colour (or the accent shifted toward `foreground`) at
//! `focus_border_width`, offset outside the bounds like a CSS `outline`. A theme that turns focus
//! outlines off (`show_focus_border = false`) turns this one off too, deliberately: the affordance
//! is the theme's call, not each caller's.
//!
//! **There is no flag to set.** The ring reads the tree, and the tree is where keyboard focus lives,
//! so the ring cannot say "the keys come here" while the keys go elsewhere. A host moves the
//! keyboard into a region the way it moves it anywhere (`focus_scope`, a press, Tab) and the ring
//! follows. It is not focusable itself, and pointer events, ticks, layout and paint pass straight
//! through, so the wrapped subtree behaves exactly as it did unwrapped.
//!
//! **The pointer is never gated.** Clicking, dragging, hovering and the wheel reach the scope
//! exactly as before. (`tests/pointer_delivery.rs` holds this to the whole pointer set.)
//!
//! ```ignore
//! // Wrap an area; it outlines itself whenever the keyboard is inside it.
//! let framed = FocusScope::new(my_container);
//! ```

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::reactive::SignalGet;
use crate::style::{Direction, Length};

/// A transparent wrapper that outlines its child while the keyboard is anywhere inside it.
pub struct FocusScope {
    base: Base,
    /// Corner radius override; otherwise the theme's control radius.
    radius: Option<f32>,
    /// Outline colour override; otherwise the theme's effective focus ring.
    color: Option<Color>,
    /// Whether the keyboard was inside at the last [`tick`](Component::tick), so a change repaints.
    seen: bool,
}

#[heca_grid_ui_macros::props]
impl FocusScope {
    /// Wrap `child`; it outlines itself while the keyboard is anywhere inside.
    pub fn new(child: impl crate::builders::IntoComponent) -> Self {
        Self::wrap(child.into_component())
    }

    fn wrap(child: Box<dyn Component>) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds are the child's (the outline is drawn off them),
        // and use a column so a single child still stretches across the cross axis — the same
        // transparency `KeyHint` needs, for the same reason.
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        base.style.layout.direction = Direction::Column;
        // …and transparent to layout as well, or a child sized as a share resolves it against this
        // wrapper and quietly becomes its content size (`component::wrap_transparently`).
        crate::component::wrap_transparently(&mut base, child.as_ref());
        base.children.push(child);
        Self {
            base,
            radius: None,
            color: None,
            seen: false,
        }
    }

    /// Corner radius of the outline in logical px (default: the theme's control radius).
    #[heca_grid_ui_macros::prop]
    pub fn radius(mut self, px: f32) -> Self {
        self.radius = Some(px);
        self
    }

    /// Override the outline colour (default: the theme's effective focus ring). Lets a host mark a
    /// different *kind* of focus distinctly while keeping `FocusScope` itself target-agnostic.
    #[heca_grid_ui_macros::prop]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

impl Component for FocusScope {
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
        // The wrapped, still-interactive child first.
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
        if !crate::focus::contains_keyboard(self) {
            return;
        }
        // Whether focus outlines are drawn at all is the theme's decision, exactly as it is for
        // every control's ring — so a theme cannot end up with rings on its buttons and none here,
        // or the other way round.
        if !cx.theme().colors.show_focus_border {
            return;
        }
        let ring = self
            .color
            .unwrap_or_else(|| cx.theme().colors.effective_focus_ring());
        let radius = self
            .radius
            .unwrap_or_else(|| cx.theme().colors.control_radius());
        cx.focus_ring(self.base.bounds, ring, radius);
    }

    fn tick(&mut self, dt: f32) -> bool {
        let focused = crate::focus::contains_keyboard(self);
        if focused != self.seen {
            self.seen = focused;
            // Only this subtree's bounds are damaged — the ring appears/disappears in place, with
            // no rebuild (the keyboard moved, and nothing else did).
            self.base.mark_needs_paint();
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}

impl LayoutExt for FocusScope {}
impl StyleExt for FocusScope {}
impl Parent for FocusScope {}

#[cfg(test)]
mod tests;
