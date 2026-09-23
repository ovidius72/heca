//! [`FocusScope`] — a **keyboard focus scope** for a whole component subtree: it takes keyboard
//! events only while it holds focus, and outlines itself while it does.
//!
//! Every focusable *control* in the library already draws its own ring
//! ([`Button`](super::Button), [`Item`](super::Item), [`IconButton`](super::IconButton) …): they own
//! their focus, so they own the affordance. What has no owner is the case where the thing that holds
//! keyboard focus is **a whole area** rather than a control — a sidebar container that the scroll
//! keys act on, a panel a mode is aimed at. Nothing in the tree can answer for that, because the
//! focus is not any single widget's: it belongs to the subtree, and only the host knows which
//! subtree has it.
//!
//! So `FocusScope` is the same shape as [`KeyHint`](super::KeyHint): a **transparent wrapper**
//! around one child, driven by a host-owned [`Signal<bool>`] (read-via-signals /
//! write-via-actions). It is not focusable itself, and pointer events, ticks, layout and paint pass
//! straight through, so the wrapped subtree behaves exactly as it did unwrapped.
//!
//! It adds exactly two things, both from that one signal:
//!
//! 1. **The keyboard.** The signal is bound to [`Base::focused`](crate::component::Base::focused),
//!    which is how the framework already decides where a key goes: keyboard events are delivered
//!    down the focus owner's ancestor chain and back up it, so an unfocused scope is simply not on
//!    the path. It has nothing to gate, nothing to decline and nothing to forward — a host sends
//!    one semantic intent into a tree of scopes and the focused one is the only one it reaches.
//!    (This wrapper used to own its subtree's whole event walk to achieve that, and skip it per
//!    event kind.)
//! 2. **The outline.** While focused it draws the theme's focus ring around the child's bounds —
//!    [`PaintCx::focus_ring`], the `focus_ring` colour (or the accent shifted toward `foreground`)
//!    at `focus_border_width`, offset outside the bounds like a CSS `outline`. A theme that turns
//!    focus outlines off (`show_focus_border = false`) turns this one off too, deliberately: the
//!    affordance is the theme's call, not each caller's.
//!
//! The two halves share the signal on purpose — a ring that says "the keys come here" while the
//! keys go elsewhere is worse than no ring.
//!
//! **The pointer is never gated.** Clicking, dragging, hovering and the wheel reach an unfocused
//! scope exactly as before; the mouse carries its own target, so it needs no focus to say where it
//! meant. (`tests/pointer_delivery.rs` holds this to the whole pointer set.)
//!
//! ```ignore
//! // The host owns the signal and sets it when its focus moves.
//! let focused = signal(false);
//! let scope = FocusScope::new(my_container).focus(focused);
//! // …later, from an action: the ring appears AND the keys start arriving.
//! focused.set(true);
//! ```

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::reactive::{Signal, SignalGet, signal};
use crate::style::{Direction, Length};

/// A transparent wrapper that gates keyboard events on, and outlines, a host-driven focus signal.
pub struct FocusScope {
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
impl FocusScope {
    /// Wrap `child`. Bind the focus state with [`focus`](FocusScope::focus).
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
            focused: signal(false),
            radius: None,
            color: None,
            seen: false,
        }
    }

    /// Bind the **host-owned** focus signal. The host sets it when its keyboard focus moves, so
    /// key, mouse and RPC all drive the affordance through one path.
    ///
    /// It is bound to [`Base::focused`] as well, and that is the whole of the gate: keyboard
    /// events are delivered down the focus owner's ancestor chain, so an unfocused scope is not on
    /// the path and is never offered a key. There is nothing here to decline, and nothing to
    /// declare.
    #[heca_grid_ui_macros::host_only("bound to a live host signal, which static data cannot drive")]
    pub fn focus(mut self, focused: Signal<bool>) -> Self {
        self.seen = focused.get_untracked();
        self.focused = focused;
        self.base.focused = focused;
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
        let radius = self
            .radius
            .unwrap_or_else(|| cx.theme().colors.control_radius());
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

impl LayoutExt for FocusScope {}
impl StyleExt for FocusScope {}
impl Parent for FocusScope {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::WidgetIntent;
    use crate::component::{Event, Handled};
    use crate::event::PointerButton;
    use crate::layout::LayoutEngine;
    use crate::reactive::SignalUpdate;
    use crate::scene::{DrawCommand, RectCmd, Scene};
    use crate::theme::Theme;
    use crate::widgets::Flex;
    use heca_core::layout::Size;

    /// Paint a wrapper and return the rects it drew, so the outline can be looked for by colour.
    fn painted(ring: &mut FocusScope, theme: &Theme) -> Vec<RectCmd> {
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
        Flex::column().width(120.0).height(60.0)
    }

    #[test]
    fn an_unfocused_wrapper_draws_nothing_of_its_own() {
        let theme = Theme::default();
        let mut ring = FocusScope::new(child());
        assert!(
            painted(&mut ring, &theme).is_empty(),
            "a wrapper that has no focus must be invisible",
        );
    }

    #[test]
    fn a_focused_wrapper_outlines_the_child_in_the_themes_focus_colour() {
        let theme = Theme::default();
        let focused = signal(true);
        let mut ring = FocusScope::new(child()).focus(focused);
        let rects = painted(&mut ring, &theme);
        let want = theme.colors.effective_focus_ring();
        assert!(
            rects.iter().any(|r| r.border.is_some_and(
                |b| b.color == want && (b.width - theme.focus_border_width).abs() < 0.01
            )),
            "the outline is the theme's focus ring at its focus width: {rects:?}",
        );
    }

    #[test]
    fn the_outline_follows_the_signal_without_a_rebuild() {
        let theme = Theme::default();
        let focused = signal(false);
        let mut ring = FocusScope::new(child()).focus(focused);
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
        let mut ring = FocusScope::new(child()).focus(signal(true));
        assert!(
            painted(&mut ring, &theme).is_empty(),
            "the affordance is the theme's call, uniformly",
        );
    }

    #[test]
    fn a_colour_override_wins_over_the_theme() {
        let theme = Theme::default();
        let mine = Color::new(0x40, 0xe0, 0xff, 0xff);
        let mut ring = FocusScope::new(child()).focus(signal(true)).color(mine);
        let rects = painted(&mut ring, &theme);
        assert!(
            rects
                .iter()
                .any(|r| r.border.is_some_and(|b| b.color == mine)),
            "an explicit colour marks a distinct kind of focus: {rects:?}",
        );
    }

    #[test]
    fn the_wrapper_hugs_its_child_so_the_outline_lands_on_it() {
        let mut ring = FocusScope::new(child()).focus(signal(true));
        LayoutEngine::new().compute(&mut ring, Size::new(200.0, 100.0));
        let outer = ring.base().bounds;
        let inner = ring.base().children[0].base().bounds;
        assert_eq!(
            (outer.size.w, outer.size.h),
            (inner.size.w, inner.size.h),
            "a transparent wrapper measures exactly its child",
        );
    }

    // ── The gate ────────────────────────────────────────────────────────────────────────────

    /// Records the events that actually reached it, and consumes nothing.
    struct Probe {
        base: Base,
        seen: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl Probe {
        fn new() -> (Self, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
            let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let mut base = Base::new();
            base.style.layout.width = Length::Px(120.0);
            base.style.layout.height = Length::Px(60.0);
            (
                Self {
                    base,
                    seen: seen.clone(),
                },
                seen,
            )
        }
    }

    impl Component for Probe {
        fn base(&self) -> &Base {
            &self.base
        }
        fn base_mut(&mut self) -> &mut Base {
            &mut self.base
        }
        fn on_event(&mut self, ev: &Event) -> Handled {
            self.seen.borrow_mut().push(format!("{ev:?}"));
            // `No` on purpose: "it arrived" is the claim, and a widget that ignores an event must
            // not stop its siblings from seeing it.
            Handled::No
        }
    }

    /// A scope wrapping content that does **not** take the keyboard — the wrapper alone is bound.
    fn scope(focused: bool) -> (FocusScope, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
        let (probe, seen) = Probe::new();
        (FocusScope::new(probe).focus(signal(focused)), seen)
    }

    /// **A dock, as a host actually builds one**: ONE signal bound to the wrapper (which draws the
    /// ring) *and* to the widget inside that answers the keys — the same pairing as
    /// `FocusScope::focus` beside [`ScrollRegion::keyboard_target`](super::ScrollRegion::keyboard_target).
    ///
    /// That is what makes the content the focus owner, and keyboard events go to the owner. Binding
    /// only the wrapper is a real state, tested above: the ring is drawn and the wrapper answers,
    /// which is right, because nothing inside claimed the keyboard.
    fn dock(focused: bool) -> (FocusScope, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
        let (mut probe, seen) = Probe::new();
        let sig = signal(focused);
        probe.base_mut().focused = sig;
        (FocusScope::new(probe).focus(sig), seen)
    }

    /// A widget intent reaches the widget that holds the keyboard.
    #[test]
    fn a_widget_intent_reaches_the_focused_content() {
        let (mut dock, seen) = dock(true);
        crate::component::dispatch(&mut dock, &Event::Widget(WidgetIntent::ScrollPageDown));
        assert_eq!(seen.borrow().len(), 1, "the focus owner answered");
    }

    /// …and not an unfocused one's.
    #[test]
    fn a_widget_intent_does_not_enter_an_unfocused_scope() {
        let (mut scope, seen) = dock(false);
        crate::component::dispatch(&mut scope, &Event::Widget(WidgetIntent::ScrollPageDown));
        assert!(
            seen.borrow().is_empty(),
            "an unfocused scope is inert to keys"
        );
    }

    /// **The load-bearing one.** Two docks side by side: only the one holding the keyboard answers,
    /// whatever the document order — the intent is routed to it, not offered to the region and
    /// taken by whoever is reached first.
    #[test]
    fn only_the_focused_dock_answers_whatever_the_document_order() {
        let (unfocused, quiet) = dock(false);
        let (focused, heard) = dock(true);
        // Document order: the inert one FIRST, which is the arrangement that breaks under a walk.
        let mut region = Flex::column().child(unfocused).child(focused);

        let handled =
            crate::component::dispatch(&mut region, &Event::Widget(WidgetIntent::ScrollPageDown));

        assert!(
            quiet.borrow().is_empty(),
            "the unfocused dock stayed out of it"
        );
        assert_eq!(heard.borrow().len(), 1, "and the focused one was reached");
        // The probe declines, so nothing claims it — what matters is that the walk got there.
        assert_eq!(handled, Handled::No);
    }

    /// **A key is not gated like an intent — it stops at the focus owner.**
    ///
    /// An intent names a capability, so it is offered to the focused region and whatever inside it
    /// owns that capability answers. A raw key belongs to *one* widget: it reaches the focus owner
    /// (this scope) and bubbles from there, and it does **not** descend into the subtree. That is
    /// the rule that stops the first row in a list eating an Enter meant for the row the cursor is
    /// on — the bug seven widgets used to patch by hand.
    #[test]
    fn a_key_stops_at_the_focus_owner_and_does_not_enter_its_subtree() {
        let (mut open, heard) = scope(true);
        let (mut shut, quiet) = scope(false);
        let key = Event::Key {
            key: crate::component::GridKey::Enter,
            pressed: true,
        };
        crate::component::dispatch(&mut open, &key);
        crate::component::dispatch(&mut shut, &key);
        assert!(
            heard.borrow().is_empty(),
            "the key is the scope's, not its content's"
        );
        assert!(
            quiet.borrow().is_empty(),
            "and an unfocused scope hears nothing at all"
        );
    }

    /// **The pointer is never gated.** The mouse carries its own target, so it needs no focus to
    /// say where it meant — and a click on an unfocused dock is how you focus it in the first place.
    /// (`tests/pointer_delivery.rs` holds the whole set to this.)
    #[test]
    fn the_pointer_reaches_an_unfocused_scope() {
        let (mut scope, seen) = scope(false);
        LayoutEngine::new().compute(&mut scope, Size::new(200.0, 100.0));
        let pos = heca_core::layout::Point::new(10.0, 10.0);
        for ev in [
            Event::pointer_moved(pos),
            Event::pointer_pressed(pos, PointerButton::Left),
            Event::pointer_released(pos, PointerButton::Left),
            Event::wheel(pos, 0.0, 1.0),
        ] {
            crate::component::dispatch(&mut scope, &ev);
        }
        let seen = seen.borrow();
        let kinds: Vec<&str> = seen
            .iter()
            .map(|s| s.split('(').next().unwrap_or(s))
            .collect();
        for want in [
            "PointerEnter",
            "PointerMove",
            "PointerDown",
            "PointerUp",
            "Click",
            "Scroll",
        ] {
            assert!(
                kinds.contains(&want),
                "{want} entered an unfocused scope: {kinds:?}",
            );
        }
    }

    /// The keyboard follows the signal, with no rebuild — the same flip that shows the ring.
    #[test]
    fn the_keyboard_follows_the_signal() {
        let (mut probe, seen) = Probe::new();
        let focused = signal(false);
        probe.base_mut().focused = focused;
        let mut scope = FocusScope::new(probe).focus(focused);
        crate::component::dispatch(&mut scope, &Event::Widget(WidgetIntent::ScrollPageDown));
        assert!(seen.borrow().is_empty());
        focused.set(true);
        crate::component::dispatch(&mut scope, &Event::Widget(WidgetIntent::ScrollPageDown));
        assert_eq!(
            seen.borrow().len(),
            1,
            "one signal governs the ring and the keys"
        );
    }

    #[test]
    fn a_boxed_subtree_can_be_wrapped() {
        // What a chrome provider's render seam returns: a `Box<dyn Component>`, which is not itself
        // `Component`, so it cannot go through `new`.
        let body: Box<dyn Component> = Box::new(child());
        let mut ring = FocusScope::new(body).focus(signal(true));
        let theme = Theme::default();
        assert!(!painted(&mut ring, &theme).is_empty());
    }
}
