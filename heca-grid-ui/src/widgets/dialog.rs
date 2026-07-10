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
//! [`FocusManager`](crate::focus::FocusManager) over the panel subtree with the **universal
//! focus-nav set**: Tab / Shift+Tab, the arrow keys (→/↓ next, ←/↑ prev), and Ctrl+h/k (prev)
//! / Ctrl+l/j (next) all move focus among the body field(s) and buttons; Enter / Space
//! activate the focused one (firing its `on_click`),
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
    //    Modifier-aware (Shift+Tab, Ctrl+h/j/k/l) via the tracked `mods`, so the host never
    //    drives the modal — it only forwards keys + `ModifiersChanged`. Keys are routed
    //    **field-first**: Esc/Tab/Ctrl-motion are owned by the dialog; every other key is handed
    //    to the focused descendant first (so a text `Input` body types + moves its caret), and
    //    only an *unconsumed* key falls back to container behaviour (Enter → primary action,
    //    arrows → focus motion). ──

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

    /// Fire the **primary** (first) action button — used when Enter is pressed from a field
    /// that didn't consume it (e.g. a focused text input), so Enter submits like clicking OK.
    /// When a button itself is focused, Enter is delivered straight to it (it consumes it), so
    /// this only runs for the non-button-focused case.
    fn activate_primary(&mut self) {
        if !self.has_actions {
            return;
        }
        let panel = self.base.children[0].base_mut();
        // Panel layout is `[title, body?, action-row]`; the action row is the last child, and
        // its first child is the primary (leftmost) button.
        if let Some(row) = panel.children.last_mut()
            && let Some(primary) = row.base_mut().children.first_mut()
        {
            let _ = primary.event(&Event::Key { key: GridKey::Enter, pressed: true });
        }
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
                    // carries the overlay-control action). Trapped dispatch: a press on the
                    // panel *body* (no button under the cursor) keeps the focused button's
                    // ring instead of clearing it — focus is trapped inside the modal.
                    let panel = self.base.children[0].as_mut();
                    self.focus.dispatch_trapped(panel, ev);
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
            // Track modifier state (broadcast by the host) and forward it to the panel so a
            // focused field's own modifier-aware editing sees it too. Not consumed — a broadcast.
            Event::ModifiersChanged(m) => {
                self.mods = *m;
                let panel = self.base.children[0].as_mut();
                let _ = panel.event(ev);
                Handled::No
            }
            Event::Key { key, pressed: true } => {
                // The dialog owns only Esc (cancel) and Tab (focus traversal) — keys that no text
                // field needs. Everything else is field-first (below).
                if matches!(key, GridKey::Escape) {
                    self.fire_dismiss();
                    return Handled::Yes;
                }
                if matches!(key, GridKey::Tab) {
                    if self.mods.shift {
                        self.focus_prev();
                    } else {
                        self.focus_next();
                    }
                    return Handled::Yes;
                }
                // **Field-first**: hand every other key to the focused widget so it keeps ALL its
                // native behaviour — an `Input`'s typing, Ctrl+h delete, Ctrl/Cmd+A select-all,
                // caret motion, word/line select — with zero re-declaration here. The widget reads
                // its own modifiers from the broadcast `ModifiersChanged`. If it consumes the key,
                // we're done.
                let panel = self.base.children[0].as_mut();
                if self.focus.deliver_key(panel, *key) == Handled::Yes {
                    return Handled::Yes;
                }
                // Only keys the focused widget ignored fall back to container focus-nav: a button
                // ignores Enter-vs-arrows the way we want, and a text field ignores Ctrl+j/k (so
                // those still navigate) but consumes Ctrl+h (so that stays "delete"). Enter from a
                // non-consuming field submits the primary action (like clicking OK).
                match key {
                    GridKey::Enter => self.activate_primary(),
                    GridKey::ArrowRight | GridKey::ArrowDown => self.focus_next(),
                    GridKey::ArrowLeft | GridKey::ArrowUp => self.focus_prev(),
                    GridKey::Char('l' | 'j') if self.mods.ctrl => self.focus_next(),
                    GridKey::Char('h' | 'k') if self.mods.ctrl => self.focus_prev(),
                    _ => {}
                }
                Handled::Yes // swallow — the dialog is modal
            }
            // Swallow scroll while the dialog owns input (modal).
            Event::Scroll { .. } => Handled::Yes,
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

    /// Buttons in the action row that currently hold focus (drives the focus ring).
    fn focused_buttons(d: &Dialog) -> Vec<usize> {
        use crate::reactive::SignalGet;
        let panel = &d.base().children[0];
        let row = &panel.base().children[2];
        row.base()
            .children
            .iter()
            .enumerate()
            .filter(|(_, b)| b.base().focused.get_untracked())
            .map(|(i, _)| i)
            .collect()
    }

    #[test]
    fn clicking_panel_body_keeps_button_focus() {
        // Regression: a press on the panel *body* (not a button) must NOT clear the
        // focused button's ring — focus is trapped inside the modal. Previously the
        // panel dispatch cleared focus on any click that missed every focusable.
        let mut d = open_dialog();
        crate::LayoutEngine::new().compute(&mut d, Size::new(600.0, 400.0));
        // Move focus by keyboard so a button shows the focus ring.
        let _ = d.event(&Event::Key { key: GridKey::Tab, pressed: true });
        let before = focused_buttons(&d);
        assert!(!before.is_empty(), "a button is focused after Tab");

        // Click the panel body: inside the panel bounds but on the title band (no button).
        let panel = d.panel_bounds();
        let body = Point::new(panel.loc.x + panel.size.w * 0.5, panel.loc.y + 2.0);
        assert!(panel.contains(body), "test point is inside the panel body");
        let _ = d.event(&Event::PointerPressed { pos: body });

        assert_eq!(
            focused_buttons(&d),
            before,
            "clicking the panel body keeps the focused button (trapped focus)",
        );
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

    #[test]
    fn universal_focus_nav_keys_move_focus() {
        // Arrow keys and Ctrl+vim keys move focus among the buttons, like Tab — the universal
        // focus-nav set. Each is handled (consumed) and lands focus on a button.
        for key in [GridKey::ArrowDown, GridKey::ArrowUp, GridKey::ArrowLeft, GridKey::ArrowRight] {
            let mut d = open_dialog();
            assert_eq!(d.event(&Event::Key { key, pressed: true }), Handled::Yes);
            assert!(!focused_buttons(&d).is_empty(), "{key:?} focuses a button");
        }
        // Ctrl+h/j/k/l require the host to have broadcast the Ctrl modifier first.
        for key in ['h', 'j', 'k', 'l'] {
            let mut d = open_dialog();
            let _ = d.event(&Event::ModifiersChanged(Modifiers { ctrl: true, ..Default::default() }));
            assert_eq!(
                d.event(&Event::Key { key: GridKey::Char(key), pressed: true }),
                Handled::Yes,
                "Ctrl+{key} moves focus",
            );
            assert!(!focused_buttons(&d).is_empty(), "Ctrl+{key} focuses a button");
        }
    }

    #[test]
    fn text_input_body_types_and_enter_submits_primary() {
        use crate::widgets::Input;
        let fired = Rc::new(Cell::new(false));
        let f = fired.clone();
        // A form dialog: an Input body + OK (primary) / Cancel. `open` focuses the Input first.
        let mut d = Dialog::new("Rename pane")
            .body(Input::new().value("term"))
            .action(Button::new("OK").on_click(move || f.set(true)))
            .action(Button::new("Cancel"))
            .open(true);
        // A typed character is forwarded to (and consumed by) the focused input — not swallowed.
        assert_eq!(
            d.event(&Event::Key { key: GridKey::Char('x'), pressed: true }),
            Handled::Yes,
            "the focused input receives typed characters",
        );
        // Enter from the focused input submits the PRIMARY action (OK), not Cancel.
        assert!(!fired.get());
        let _ = d.event(&Event::Key { key: GridKey::Enter, pressed: true });
        assert!(fired.get(), "Enter from the input fires the primary action (OK)");
    }

    #[test]
    fn field_first_ctrl_h_edits_input_ctrl_j_navigates() {
        use crate::widgets::Input;
        // Field-first: a focused input keeps its OWN Ctrl-keys; only keys it ignores fall back
        // to dialog focus-nav. The input starts focused (body precedes the buttons).
        let mut d = Dialog::new("Rename")
            .body(Input::new().value("ab"))
            .action(Button::new("OK"))
            .action(Button::new("Cancel"))
            .open(true);
        let _ = d.event(&Event::ModifiersChanged(Modifiers { ctrl: true, ..Default::default() }));
        // Ctrl+h is the input's delete-backward → consumed by the input; focus stays on it (no
        // button focused), i.e. the dialog does NOT steal it for nav.
        let _ = d.event(&Event::Key { key: GridKey::Char('h'), pressed: true });
        assert!(focused_buttons(&d).is_empty(), "Ctrl+h stays in the input");
        // Ctrl+j is not an input editing key → falls back to dialog focus-nav (moves to a button).
        let _ = d.event(&Event::Key { key: GridKey::Char('j'), pressed: true });
        assert!(!focused_buttons(&d).is_empty(), "Ctrl+j navigates to a button");
    }
}
