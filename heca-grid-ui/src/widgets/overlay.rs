//! [`Overlay`] — the base overlay surface: scrim + panel chrome + viewport
//! positioning, shared by every overlay widget.
//!
//! **Blocking is a property of this layer, not a per-widget reimplementation**
//! (the T009 overlay rework): a *blocking* overlay paints a dimming scrim over
//! the whole viewport and swallows outside input (modal — [`Dialog`](super::Dialog));
//! a non-blocking one lets outside input fall through (light-dismiss popovers).
//! Positioning lives here once: the base fills the viewport (`Pct(1.0)`²) and
//! **centers** its single panel child with real taffy layout, so every
//! descendant gets true bounds (hint picker + pointer hit-testing need them).
//! (Anchor-to-rect positioning for dropdown/popover/tooltip specializations is
//! the planned extension — those widgets still own their placement today.)
//!
//! **Composition, not inheritance.** A specialized overlay widget ([`Dialog`](super::Dialog))
//! *composes* an `Overlay` as its subtree — the `Overlay` owns presentation
//! (scrim, shadow, panel fill, bracket reticle) and geometry
//! ([`overlay_occludes`](Component::overlay_occludes)); the specialization owns
//! its content and behaviour (focus trap, keyboard, dismissal policy) and
//! intercepts events *before* the `Overlay`'s own standalone handling runs.
//! Used directly (a host mounting an arbitrary — e.g. `realize`d — panel), the
//! `Overlay`'s own event handling provides the standard layer semantics:
//! nested-overlay-first routing, outside-click callback, blocking swallow.
//!
//! Paint goes through [`PaintCx::with_overlay`], so an overlay opened *inside*
//! this panel (a `Select` dropdown in a modal body) records a **deeper** scene
//! segment and composites above everything this overlay draws — see
//! [`Scene::overlay_segments`](crate::scene::Scene::overlay_segments).

use crate::builders::LayoutExt;
use crate::component::{paint_child, Base, Component, Event, Handled, PaintCx};
use crate::focus::FocusManager;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::Shadow;
use crate::style::{Align, Justify, Length};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

/// Multiplier on the theme `shadow.blur` token — an overlay panel is large and
/// wants a wider, softer halo than the small-surface base token.
const SHADOW_BLUR_MULT: f32 = 4.0;
/// Downward shadow offset lifting the panel off the scrim/page.
const SHADOW_DROP: f32 = 12.0;

/// The base overlay surface: a viewport-filling, centering layer that paints a
/// panel (its single child) with the shared overlay chrome — optional scrim,
/// drop shadow, theme surface fill, and the bracket reticle.
///
/// Build with [`Overlay::new`], hand it the panel via [`panel`](Overlay::panel)
/// (or [`panel_boxed`](Overlay::panel_boxed) for a mapper-produced
/// `Box<dyn Component>`), and drive visibility through
/// [`open_signal`](Overlay::open_signal). [`blocking`](Overlay::blocking)
/// selects the layer policy: blocking (default) = scrim + swallow outside
/// input; non-blocking = outside input falls through (light dismiss).
pub struct Overlay {
    base: Base,
    open: Signal<bool>,
    /// Blocking layer policy: scrim + swallow outside input (modal). `false` ⇒
    /// no scrim; outside input falls through after the outside-click callback.
    blocking: bool,
    /// Fired when a press lands outside the panel — the standalone dismissal
    /// hook (a composing widget usually implements its own policy instead).
    on_outside_click: Option<Box<dyn Fn()>>,
    /// Last-seen viewport, cached during paint for the scrim rect.
    viewport: Cell<Size>,
}

impl Overlay {
    /// A new (closed) blocking overlay with an empty panel slot.
    pub fn new() -> Self {
        let mut base = Base::new();
        // Fill the viewport and center the panel on both axes — real taffy
        // centering, so every descendant gets true bounds.
        base.style.width = Length::Pct(1.0);
        base.style.height = Length::Pct(1.0);
        base.style.justify = Justify::Center;
        base.style.align = Align::Center;
        Self {
            base,
            open: signal(false),
            blocking: true,
            on_outside_click: None,
            viewport: Cell::new(Size::new(f64::INFINITY, f64::INFINITY)),
        }
    }

    /// Set the **panel** — the single child this layer centers and decorates.
    /// The caller owns the panel's internal layout (padding, gaps, children);
    /// the overlay owns the chrome around it. Replaces any previous panel.
    pub fn panel(mut self, panel: impl Component + 'static) -> Self {
        self.base.children.clear();
        self.base.children.push(Box::new(panel));
        self
    }

    /// Like [`panel`](Overlay::panel) but takes an already-boxed component —
    /// for a panel produced by a mapper returning `Box<dyn Component>` (e.g.
    /// `heca`'s `realize(ViewNode)`).
    pub fn panel_boxed(mut self, panel: Box<dyn Component>) -> Self {
        self.base.children.clear();
        self.base.children.push(panel);
        self
    }

    /// Layer policy: `true` (default) = modal — dimming scrim + outside input
    /// swallowed; `false` = light layer — no scrim, outside input falls through.
    pub fn blocking(mut self, blocking: bool) -> Self {
        self.blocking = blocking;
        self
    }

    /// Set the initial open state.
    pub fn open(self, open: bool) -> Self {
        self.open.set(open);
        self
    }

    /// The open-state signal — the host (or composing widget) binds this.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    /// Called when a press lands **outside** the panel (standalone use; a
    /// composing widget usually intercepts the press and applies its own
    /// dismissal policy instead).
    pub fn on_outside_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_outside_click = Some(Box::new(f));
        self
    }

    /// The panel's laid-out bounds (valid after layout; zero before).
    pub fn panel_bounds(&self) -> Rectangle {
        self.base
            .children
            .first()
            .map(|c| c.base().bounds)
            .unwrap_or_else(|| Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)))
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }
}

impl Default for Overlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Overlay {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable only while open, so a host's overlay scan can route input here
    /// when the `Overlay` is mounted directly (a composing widget like `Dialog`
    /// is found first in pre-order and intercepts instead).
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// A **blocking** overlay occludes the whole viewport (its scrim owns every
    /// point); a non-blocking one occludes only the panel itself.
    fn overlay_occludes(&self, pos: Point) -> bool {
        self.is_open() && (self.blocking || self.panel_bounds().contains(pos))
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || !self.is_open() {
            return;
        }
        self.viewport.set(cx.viewport());
        let (background, surface, scrim_a, shadow, shadow_blur, radius) = {
            let t = cx.theme();
            (
                t.colors.background,
                t.colors.surface,
                t.colors.interaction.scrim,
                t.shadow_color(),
                t.colors.shadow.blur,
                t.colors.border_radius,
            )
        };
        let panel = self.panel_bounds();

        cx.with_overlay(|cx| {
            // Scrim over the whole viewport — the visual half of the blocking
            // layer policy (the event half swallows outside input below).
            if self.blocking {
                let vp = self.viewport.get();
                let scrim = if vp.w.is_finite() {
                    Rectangle::new(Point::new(0.0, 0.0), vp)
                } else {
                    panel
                };
                cx.rect(scrim, background.with_alpha(scrim_a), None, 0.0, None);
            }

            // Lift the panel, fill it, stamp the shared bracket reticle (same
            // visual language as Pane / DockFrame).
            cx.drop_shadow(
                panel,
                radius,
                Shadow {
                    color: shadow,
                    radius: shadow_blur * SHADOW_BLUR_MULT,
                    dx: 0.0,
                    dy: SHADOW_DROP,
                },
            );
            cx.rect(panel, surface, None, radius, None);
            cx.bracket_frame(panel);

            // The panel's real children on top of the fill. A nested overlay
            // painted in here records a DEEPER scene segment → composites above
            // everything this layer draws (Scene::overlay_segments).
            for child in &self.base.children {
                paint_child(child.as_ref(), cx);
            }
        });
    }

    /// **Standalone** layer semantics (a composing widget intercepts events
    /// before this runs and applies its own policy — see the module docs):
    /// nested-overlay-first routing, outside-click callback, blocking swallow.
    fn event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        // Nested overlay first: an open overlay INSIDE the panel (a Select
        // dropdown) captures input over the whole layer before the panel's
        // ordinary children see it — mirroring the host's focus.rs overlay
        // scan. `offer_to_overlay` carries no manager state, so a fresh
        // FocusManager is just the scan.
        let panel_root = match self.base.children.first_mut() {
            Some(p) => p.as_mut(),
            None => return if self.blocking { Handled::Yes } else { Handled::No },
        };
        if FocusManager::new().offer_to_overlay(panel_root, ev) == Handled::Yes {
            return Handled::Yes;
        }
        let panel = self.panel_bounds();
        match ev {
            Event::PointerPressed { pos } => {
                if panel.contains(*pos) {
                    let panel_root = self.base.children[0].as_mut();
                    let _ = panel_root.event(ev);
                } else if let Some(f) = &self.on_outside_click {
                    f();
                }
                if self.blocking { Handled::Yes } else { Handled::No }
            }
            Event::PointerMoved { .. } => {
                let panel_root = self.base.children[0].as_mut();
                let _ = panel_root.event(ev);
                if self.blocking { Handled::Yes } else { Handled::No }
            }
            // A blocking layer owns the wheel too (the page behind must not
            // scroll); a nested scrollable inside the panel already had first
            // dibs via the overlay scan above.
            Event::Scroll { .. } => {
                if self.blocking { Handled::Yes } else { Handled::No }
            }
            _ => Handled::No,
        }
    }
}

impl LayoutExt for Overlay {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::Parent;
    use crate::widgets::{Flex, Label};

    fn open_overlay() -> Overlay {
        Overlay::new()
            .panel(Flex::column().child(Label::new("hi")))
            .open(true)
    }

    #[test]
    fn closed_overlay_is_inert() {
        let mut o = Overlay::new().panel(Flex::column());
        assert!(!o.overlay_active());
        assert!(!o.focusable());
        assert!(!o.overlay_occludes(Point::new(1.0, 1.0)));
        assert_eq!(
            o.event(&Event::PointerPressed { pos: Point::new(1.0, 1.0) }),
            Handled::No
        );
    }

    #[test]
    fn blocking_overlay_occludes_everywhere_and_swallows_outside_input() {
        let mut o = open_overlay();
        assert!(o.overlay_occludes(Point::new(-500.0, -500.0)), "scrim owns every point");
        assert_eq!(
            o.event(&Event::PointerPressed { pos: Point::new(-500.0, -500.0) }),
            Handled::Yes,
            "modal swallows the outside press"
        );
        assert_eq!(o.event(&Event::Scroll { delta_x: 0.0, delta_y: 1.0 }), Handled::Yes);
    }

    #[test]
    fn non_blocking_overlay_occludes_only_its_panel_and_lets_outside_fall_through() {
        use std::cell::Cell;
        use std::rc::Rc;
        let dismissed = Rc::new(Cell::new(false));
        let d = dismissed.clone();
        let mut o = Overlay::new()
            .blocking(false)
            .panel(Flex::column().child(Label::new("hi")))
            .open(true)
            .on_outside_click(move || d.set(true));
        // Give the panel real bounds (as layout would).
        o.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(100.0, 100.0), Size::new(50.0, 20.0));
        assert!(o.overlay_occludes(Point::new(110.0, 110.0)), "panel point occludes");
        assert!(!o.overlay_occludes(Point::new(0.0, 0.0)), "outside point does not");
        assert_eq!(
            o.event(&Event::PointerPressed { pos: Point::new(0.0, 0.0) }),
            Handled::No,
            "light layer lets the outside press fall through"
        );
        assert!(dismissed.get(), "outside press fired the dismissal hook");
    }
}
