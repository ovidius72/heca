//! [`Dialog`] — a centered overlay **panel that holds real child components**.
//!
//! Unlike [`Modal`](super::Modal) — which draws its title/message/buttons **manually** and
//! therefore has no child subtree (so its buttons can't be hint targets, focus-traversed as
//! components, or hold arbitrary content) — `Dialog` is a true container: it lays out a
//! centered panel over a dimming scrim and paints its **children** (a title, an arbitrary
//! `body`, and a row of real action [`Button`](super::Button)s). Because the buttons are
//! real components, they participate in the universal hint picker, standard focus traversal,
//! and pointer routing for free.
//!
//! It uses the same overlay contract as `Modal`/[`Select`](super::Select): it reports
//! [`overlay_active`](Component::overlay_active) + [`focusable`](Component::focusable) while
//! open, so the host routes input here first. Keyboard is handled by an embedded
//! [`FocusManager`](crate::focus::FocusManager) over the panel subtree: Tab / ← / →  move
//! focus among the buttons, Enter / Space activate the focused one (firing its `on_click`),
//! and Esc or a scrim click fire the [`on_dismiss`](Dialog::on_dismiss) callback (there are no
//! result closures baked in; a button's own `on_click` carries the action, and dismissal is a
//! callback the host points at its own overlay-close path).
//!
//! Centering is real layout: the root fills the viewport (`Pct(1.0)` × `Pct(1.0)`) with
//! `Justify::Center` + `Align::Center`, so the single panel child is centered by taffy and
//! every descendant gets true bounds (which the hint picker and pointer hit-testing need).

use crate::builders::{LayoutExt, Parent};
use crate::component::{paint_child, Base, Component, Event, GridKey, Handled, Modifiers, PaintCx};
use crate::focus::FocusManager;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::Shadow;
use crate::style::{Align, Justify, Length};
use crate::widgets::{Flex, Label};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

/// Panel inner padding.
const PAD: f32 = 18.0;
/// Gap between the title, body, and the action row.
const GAP: f32 = 14.0;
/// Gap between adjacent action buttons.
const BTN_GAP: f32 = 10.0;
/// Multiplier on the theme `shadow.blur` token — the panel is large and wants a wider,
/// softer halo than the small-surface base token (mirrors [`Modal`](super::Modal)).
const SHADOW_BLUR_MULT: f32 = 4.0;
/// Downward offset lifting the panel off the scrim.
const SHADOW_DROP: f32 = 12.0;

/// A centered overlay panel over a scrim, holding real child components.
///
/// Structure: the `Dialog` base is a full-viewport centering container whose single child is
/// the **panel** — a padded column of `[title, body?, action-row?]`. The action row is a
/// right-aligned [`Flex`] row of the caller's [`Button`](super::Button)s.
pub struct Dialog {
    base: Base,
    open: Signal<bool>,
    /// When `false`, Esc / scrim clicks are swallowed but don't dismiss — a forced-decision
    /// dialog (the user must pick a button). Default `true`.
    dismissible: bool,
    /// Keyboard focus across the panel's focusable descendants (the action buttons, plus any
    /// focusables inside a rich `body`).
    focus: FocusManager,
    /// Fired when Esc or a scrim click requests dismissal (only if `dismissible`). The host
    /// points this at its overlay-close path (e.g. emit `CloseOverlay`).
    on_dismiss: Option<Box<dyn Fn()>>,
    /// Live modifier state, tracked from [`Event::ModifiersChanged`] (broadcast to the tree by
    /// the host) — a key event carries no modifiers, so the widget needs this to distinguish
    /// Shift+Tab and Ctrl+h/l itself, without any host special-casing.
    mods: Modifiers,
    /// Whether an action row exists yet (created lazily on the first [`action`](Dialog::action)).
    has_actions: bool,
    /// Last-seen viewport, cached during paint for the scrim rect.
    viewport: Cell<Size>,
}

impl Dialog {
    /// A new (closed) dialog titled `title`. Add content with [`body`](Dialog::body) and
    /// buttons with [`action`](Dialog::action), in that order.
    pub fn new(title: impl Into<String>) -> Self {
        // Panel: a padded column holding the title, then body + actions as they're added.
        let panel = Flex::column()
            .padding(PAD)
            .gap(GAP)
            .child(Label::new(title));

        // Root: fill the viewport and center the panel both axes (real taffy centering, so
        // every descendant gets true bounds for hint/pointer hit-testing).
        let mut base = Base::new();
        base.style.width = Length::Pct(1.0);
        base.style.height = Length::Pct(1.0);
        base.style.justify = Justify::Center;
        base.style.align = Align::Center;
        base.children.push(Box::new(panel));

        Self {
            base,
            open: signal(false),
            dismissible: true,
            focus: FocusManager::new(),
            on_dismiss: None,
            mods: Modifiers::default(),
            has_actions: false,
            viewport: Cell::new(Size::new(f64::INFINITY, f64::INFINITY)),
        }
    }

    /// Set the dialog **body** — an arbitrary component (a message label, a form, a table…),
    /// inserted between the title and the action row. Call before [`action`](Dialog::action).
    pub fn body(mut self, body: impl Component + 'static) -> Self {
        self.panel_mut().children.push(Box::new(body));
        self
    }

    /// Like [`body`](Dialog::body) but takes an already-boxed component — for a body produced
    /// by a mapper that returns `Box<dyn Component>` (e.g. `heca`'s `realize(ViewNode)`), which
    /// can't be passed to `body` because `Box<dyn Component>` is not itself `Component`.
    pub fn body_boxed(mut self, body: Box<dyn Component>) -> Self {
        self.panel_mut().children.push(body);
        self
    }

    /// Append an action **button** (a real [`Button`](super::Button) the caller has already
    /// wired with its `on_click` + hint target). Buttons live in a right-aligned row along
    /// the panel bottom, in call order.
    pub fn action(mut self, button: impl Component + 'static) -> Self {
        if !self.has_actions {
            // Lazily create the right-aligned action row on first use.
            let row = Flex::row().gap(BTN_GAP).justify(Justify::End);
            self.panel_mut().children.push(Box::new(row));
            self.has_actions = true;
        }
        let panel = self.panel_mut();
        let row_idx = panel.children.len() - 1;
        panel.children[row_idx]
            .base_mut()
            .children
            .push(Box::new(button));
        self
    }

    /// `false` forces an explicit choice — Esc / scrim are swallowed without dismissing.
    /// Pair with a cancel button so there's always a non-destructive way out.
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Set the callback fired when Esc or a scrim click requests dismissal (respecting
    /// [`dismissible`](Dialog::dismissible)). The host wires this to its overlay-close path.
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    /// Set the initial open state (focusing the first focusable — the safe default when the
    /// caller orders `[Cancel, …, Confirm]`).
    pub fn open(mut self, open: bool) -> Self {
        self.open.set(open);
        if open {
            // Focus the safe-default (first) button so Enter works — but WITHOUT the ring; it
            // appears only once the user navigates by keyboard (focus-visible).
            let panel = self.base.children[0].as_mut();
            self.focus.focus_first_quiet(panel);
        }
        self
    }

    /// The open-state signal — the host binds this to show/hide the dialog.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    // ── Self-contained keyboard: the widget owns focus traversal + activation + dismissal.
    //    Modifier-aware (Shift+Tab, Ctrl+h/l) via the tracked `mods`, so the host never drives
    //    the modal — it only forwards keys + `ModifiersChanged`, like any other overlay. ──

    /// Move keyboard focus to the next focusable descendant (wraps).
    fn focus_next(&mut self) {
        let panel = self.base.children[0].as_mut();
        self.focus.advance(panel, true);
    }

    /// Move keyboard focus to the previous focusable descendant (wraps).
    fn focus_prev(&mut self) {
        let panel = self.base.children[0].as_mut();
        self.focus.advance(panel, false);
    }

    /// Activate the focused descendant (Enter / Space → its `on_click`).
    fn activate_focused(&mut self) {
        let panel = self.base.children[0].as_mut();
        self.focus.deliver_key(panel, GridKey::Enter);
    }

    /// Fire the dismiss callback. **Esc** always calls this (a universal modal cancel); the
    /// **scrim / outside-click** path gates it on [`dismissible`](Dialog::dismissible) — so a
    /// forced-choice dialog (`dismissible(false)`) still lets Esc cancel but ignores stray
    /// outside clicks.
    fn fire_dismiss(&self) {
        if let Some(f) = &self.on_dismiss {
            f();
        }
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// The panel container (the single child), mutably.
    fn panel_mut(&mut self) -> &mut Base {
        self.base.children[0].base_mut()
    }

    /// The panel's laid-out bounds (valid after layout; a zero rect before first layout).
    fn panel_bounds(&self) -> Rectangle {
        self.base
            .children
            .first()
            .map(|c| c.base().bounds)
            .unwrap_or_else(|| Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)))
    }
}

impl Component for Dialog {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable only while open, so the host's overlay scan routes input here.
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
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
            // Scrim over the whole viewport.
            let vp = self.viewport.get();
            let scrim = if vp.w.is_finite() {
                Rectangle::new(Point::new(0.0, 0.0), vp)
            } else {
                panel
            };
            cx.rect(scrim, background.with_alpha(scrim_a), None, 0.0, None);

            // Lift the panel off the scrim, then fill it + stamp the shared bracket reticle
            // (same visual language as Modal / Pane / DockFrame).
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

            // Real children (title, body, action buttons) on top of the panel fill.
            for child in &self.base.children {
                paint_child(child.as_ref(), cx);
            }
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        let panel_bounds = self.panel_bounds();
        match ev {
            Event::PointerPressed { pos } => {
                if panel_bounds.contains(*pos) {
                    // Focus + deliver the press to the button under the cursor (its on_click
                    // carries the overlay-control action).
                    let panel = self.base.children[0].as_mut();
                    self.focus.dispatch(panel, ev);
                } else if self.dismissible {
                    // Scrim / outside click dismisses only when dismissible.
                    self.fire_dismiss();
                }
                // Always swallow while open (modal).
                Handled::Yes
            }
            Event::PointerMoved { .. } => {
                let panel = self.base.children[0].as_mut();
                let _ = panel.event(ev); // button hover
                Handled::Yes
            }
            // Track modifier state (broadcast by the host) so Shift+Tab / Ctrl+h/l work without
            // the host special-casing the modal. Not consumed — it's a broadcast.
            Event::ModifiersChanged(m) => {
                self.mods = *m;
                Handled::No
            }
            Event::Key { key: GridKey::Escape, pressed: true } => {
                // Esc is a universal modal cancel — always dismisses (even when not dismissible).
                self.fire_dismiss();
                Handled::Yes
            }
            Event::Key { key: GridKey::Enter | GridKey::Space, pressed: true } => {
                self.activate_focused();
                Handled::Yes
            }
            // Tab moves focus — backward with Shift (the widget reads the tracked modifiers).
            Event::Key { key: GridKey::Tab, pressed: true } => {
                if self.mods.shift {
                    self.focus_prev();
                } else {
                    self.focus_next();
                }
                Handled::Yes
            }
            Event::Key { key: GridKey::ArrowRight, pressed: true } => {
                self.focus_next();
                Handled::Yes
            }
            Event::Key { key: GridKey::ArrowLeft, pressed: true } => {
                self.focus_prev();
                Handled::Yes
            }
            // Vim-style focus motion: Ctrl+h / Ctrl+l.
            Event::Key { key: GridKey::Char('h'), pressed: true } if self.mods.ctrl => {
                self.focus_prev();
                Handled::Yes
            }
            Event::Key { key: GridKey::Char('l'), pressed: true } if self.mods.ctrl => {
                self.focus_next();
                Handled::Yes
            }
            // Swallow every other key + scroll while the dialog owns input (modal).
            Event::Key { .. } | Event::Scroll { .. } => Handled::Yes,
            _ => Handled::No,
        }
    }
}

impl LayoutExt for Dialog {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{Button, Label};
    use std::rc::Rc;

    fn open_dialog() -> Dialog {
        Dialog::new("Delete pane?")
            .body(Label::new("This action cannot be undone."))
            .action(Button::new("Cancel"))
            .action(Button::new("Delete"))
            .open(true)
    }

    /// An open dialog wired with a dismiss flag, for the Esc / scrim tests.
    fn open_dialog_with_flag() -> (Dialog, Rc<Cell<bool>>) {
        let flag = Rc::new(Cell::new(false));
        let f = flag.clone();
        let d = open_dialog().on_dismiss(move || f.set(true));
        (d, flag)
    }

    #[test]
    fn structure_is_panel_with_title_body_and_action_row() {
        let d = open_dialog();
        // One child: the panel.
        assert_eq!(d.base().children.len(), 1);
        // Panel: [title, body, action-row].
        let panel = &d.base().children[0];
        assert_eq!(panel.base().children.len(), 3, "title + body + action row");
        // Action row holds the two buttons.
        let row = &panel.base().children[2];
        assert_eq!(row.base().children.len(), 2, "two action buttons");
    }

    #[test]
    fn closed_dialog_ignores_input_and_is_not_overlay() {
        let mut d = Dialog::new("x").action(Button::new("OK")); // not opened
        assert!(!d.overlay_active());
        assert!(!d.focusable());
        assert_eq!(
            d.event(&Event::Key { key: GridKey::Escape, pressed: true }),
            Handled::No,
            "a closed dialog handles nothing",
        );
    }

    #[test]
    fn escape_fires_dismiss_when_dismissible() {
        let (mut d, flag) = open_dialog_with_flag();
        assert_eq!(
            d.event(&Event::Key { key: GridKey::Escape, pressed: true }),
            Handled::Yes,
        );
        assert!(flag.get(), "Esc fired the dismiss callback");
    }

    #[test]
    fn escape_dismisses_even_when_forced_but_scrim_does_not() {
        // Esc is a universal modal cancel: it fires `on_dismiss` even on a `dismissible(false)`
        // (forced-choice) dialog. Only the scrim / outside-click is gated by `dismissible`.
        let flag = Rc::new(Cell::new(false));
        let f = flag.clone();
        let mut d = open_dialog().dismissible(false).on_dismiss(move || f.set(true));
        assert_eq!(
            d.event(&Event::Key { key: GridKey::Escape, pressed: true }),
            Handled::Yes,
        );
        assert!(flag.get(), "Esc cancels even a forced dialog");

        // A scrim click (press outside the panel) on a forced dialog must NOT dismiss.
        flag.set(false);
        let _ = d.event(&Event::PointerPressed { pos: Point::new(-100.0, -100.0) });
        assert!(!flag.get(), "forced dialog ignores the scrim/outside click");
    }
}
