//! [`FocusRing`] — a **generic keyboard-focus outline** for any component subtree.
//!
//! Every focusable *control* in the library already draws its own ring
//! ([`Button`](super::Button), [`Item`](super::Item), [`IconButton`](super::IconButton) …): they own
//! their focus, so they own the affordance. What has no owner is the case where the thing that holds
//! keyboard focus is **a whole area** rather than a control — a sidebar container that the scroll
//! keys act on, a panel a mode is aimed at. Nothing in the tree can draw that, because the focus is
//! not any single widget's: it belongs to the subtree, and only the host knows which subtree has it.
//!
//! So `FocusRing` is the same shape as [`KeyHint`](super::KeyHint): a **transparent wrapper** around
//! one child, driven by a host-owned [`Signal<bool>`] (read-via-signals / write-via-actions). It is
//! not focusable itself and routes events/ticks straight through, so the wrapped subtree behaves
//! exactly as it did unwrapped. It only *adds paint*: while the signal is `true` it draws the
//! theme's focus outline around the child's bounds, and nothing at all while it is `false`.
//!
//! It is **the same outline every control uses** — [`PaintCx::focus_ring`], the theme's
//! `focus_ring` colour (or the accent shifted toward `foreground`) at `focus_border_width`,
//! offset outside the bounds like a CSS `outline`. A theme that turns focus outlines off
//! (`show_focus_border = false`) turns this one off too, deliberately: the affordance is the
//! theme's call, not each caller's.
//!
//! ```ignore
//! // The host owns the signal and sets it when its focus moves.
//! let focused = signal(false);
//! let framed = FocusRing::new(my_container).focus(focused);
//! // …later, from an action:
//! focused.set(true);
//! ```

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::reactive::{Signal, SignalGet, signal};
use crate::style::{Direction, Length};

/// A transparent wrapper that outlines its child while a host-driven focus signal is on.
pub struct FocusRing {
    base: Base,
    /// Host-owned focus state: `true` draws the outline, `false` draws nothing.
    focused: Signal<bool>,
    /// Corner radius override; otherwise the theme's control radius.
    radius: Option<f32>,
    /// Outline colour override; otherwise the theme's effective focus ring.
    color: Option<Color>,
    /// Last value seen by [`tick`](Component::tick), so a change repaints.
    seen: bool,
}

#[heca_grid_ui_macros::props]
impl FocusRing {
    /// Wrap `child`. Bind the focus state with [`focus`](FocusRing::focus).
    pub fn new(child: impl Component + 'static) -> Self {
        Self::wrap(Box::new(child))
    }

    /// Wrap an **already-boxed** subtree — what a dynamically built tree is
    /// ([`realize`](crate::widgets) output, a chrome provider's render seam), where the concrete
    /// widget type is not known at the call site. Mirrors
    /// [`Parent::child_boxed`](crate::builders::Parent::child_boxed).
    pub fn new_boxed(child: Box<dyn Component>) -> Self {
        Self::wrap(child)
    }

    fn wrap(child: Box<dyn Component>) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds are the child's (the outline is drawn off them),
        // and use a column so a single child still stretches across the cross axis — the same
        // transparency `KeyHint` needs, for the same reason.
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        base.style.layout.direction = Direction::Column;
        base.children.push(child);
        Self {
            base,
            focused: signal(false),
            radius: None,
            color: None,
            seen: false,
        }
    }

    /// Bind the **host-owned** focus signal. The host sets it when its keyboard focus moves, so
    /// key, mouse and RPC all drive the affordance through one path.
    #[heca_grid_ui_macros::host_only("bound to a live host signal, which static data cannot drive")]
    pub fn focus(mut self, focused: Signal<bool>) -> Self {
        self.seen = focused.get_untracked();
        self.focused = focused;
        self
    }

    /// The focus signal (e.g. to set it directly).
    pub fn focus_signal(&self) -> Signal<bool> {
        self.focused
    }

    /// Corner radius of the outline in logical px (default: the theme's control radius).
    #[heca_grid_ui_macros::prop]
    pub fn radius(mut self, px: f32) -> Self {
        self.radius = Some(px);
        self
    }

    /// Override the outline colour (default: the theme's effective focus ring). Lets a host mark a
    /// different *kind* of focus distinctly while keeping `FocusRing` itself target-agnostic.
    #[heca_grid_ui_macros::prop]
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

impl Component for FocusRing {
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
        if !self.focused.get_untracked() {
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
        let radius = self.radius.unwrap_or_else(|| cx.theme().colors.control_radius());
        cx.focus_ring(self.base.bounds, ring, radius);
    }

    fn tick(&mut self, dt: f32) -> bool {
        let focused = self.focused.get_untracked();
        if focused != self.seen {
            self.seen = focused;
            // Only this subtree's bounds are damaged — the ring appears/disappears in place, with
            // no rebuild (the host flips a signal).
            self.base.mark_needs_paint();
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}

impl LayoutExt for FocusRing {}
impl StyleExt for FocusRing {}
impl Parent for FocusRing {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutEngine;
    use crate::reactive::SignalUpdate;
    use crate::scene::{DrawCommand, RectCmd, Scene};
    use crate::theme::Theme;
    use crate::widgets::Flex;
    use heca_core::layout::Size;

    /// Paint a wrapper and return the rects it drew, so the outline can be looked for by colour.
    fn painted(ring: &mut FocusRing, theme: &Theme) -> Vec<RectCmd> {
        LayoutEngine::new().compute(ring, Size::new(200.0, 100.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, theme);
            ring.paint(&mut cx);
        }
        scene
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCommand::Rect(r) => Some(*r),
                _ => None,
            })
            .collect()
    }

    fn child() -> Flex {
        Flex::column()
            .width(Length::Px(120.0))
            .height(Length::Px(60.0))
    }

    #[test]
    fn an_unfocused_wrapper_draws_nothing_of_its_own() {
        let theme = Theme::default();
        let mut ring = FocusRing::new(child());
        assert!(
            painted(&mut ring, &theme).is_empty(),
            "a wrapper that has no focus must be invisible",
        );
    }

    #[test]
    fn a_focused_wrapper_outlines_the_child_in_the_themes_focus_colour() {
        let theme = Theme::default();
        let focused = signal(true);
        let mut ring = FocusRing::new(child()).focus(focused);
        let rects = painted(&mut ring, &theme);
        let want = theme.colors.effective_focus_ring();
        assert!(
            rects.iter().any(|r| r
                .border
                .is_some_and(|b| b.color == want
                    && (b.width - theme.focus_border_width).abs() < 0.01)),
            "the outline is the theme's focus ring at its focus width: {rects:?}",
        );
    }

    #[test]
    fn the_outline_follows_the_signal_without_a_rebuild() {
        let theme = Theme::default();
        let focused = signal(false);
        let mut ring = FocusRing::new(child()).focus(focused);
        assert!(painted(&mut ring, &theme).is_empty());
        focused.set(true);
        assert!(
            !painted(&mut ring, &theme).is_empty(),
            "flipping the host's signal is all it takes",
        );
        focused.set(false);
        assert!(painted(&mut ring, &theme).is_empty());
    }

    #[test]
    fn a_theme_that_hides_focus_outlines_hides_this_one_too() {
        let mut theme = Theme::default();
        theme.colors.show_focus_border = false;
        let mut ring = FocusRing::new(child()).focus(signal(true));
        assert!(
            painted(&mut ring, &theme).is_empty(),
            "the affordance is the theme's call, uniformly",
        );
    }

    #[test]
    fn a_colour_override_wins_over_the_theme() {
        let theme = Theme::default();
        let mine = Color::new(0x40, 0xe0, 0xff, 0xff);
        let mut ring = FocusRing::new(child()).focus(signal(true)).color(mine);
        let rects = painted(&mut ring, &theme);
        assert!(
            rects.iter().any(|r| r.border.is_some_and(|b| b.color == mine)),
            "an explicit colour marks a distinct kind of focus: {rects:?}",
        );
    }

    #[test]
    fn the_wrapper_hugs_its_child_so_the_outline_lands_on_it() {
        let mut ring = FocusRing::new(child()).focus(signal(true));
        LayoutEngine::new().compute(&mut ring, Size::new(200.0, 100.0));
        let outer = ring.base().bounds;
        let inner = ring.base().children[0].base().bounds;
        assert_eq!(
            (outer.size.w, outer.size.h),
            (inner.size.w, inner.size.h),
            "a transparent wrapper measures exactly its child",
        );
    }

    #[test]
    fn a_boxed_subtree_can_be_wrapped() {
        // What a chrome provider's render seam returns: a `Box<dyn Component>`, which is not itself
        // `Component`, so it cannot go through `new`.
        let body: Box<dyn Component> = Box::new(child());
        let mut ring = FocusRing::new_boxed(body).focus(signal(true));
        let theme = Theme::default();
        assert!(!painted(&mut ring, &theme).is_empty());
    }
}
