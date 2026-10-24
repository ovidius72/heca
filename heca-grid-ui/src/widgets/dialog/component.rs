//! The dialog's runtime: its self-contained keyboard (focus traversal, primary
//! activation, dismissal) and the `Component` impl that reports the modal overlay
//! and routes the pointer and keys around the focus trap.

use super::*;
use crate::component::{Component, Event, GridKey, Handled, WidgetIntent};
use crate::reactive::SignalGet;
use heca_core::layout::{Point, Rectangle, Size};

// ── Self-contained keyboard: the widget owns focus traversal + activation + dismissal.
//    Modifier-aware (Shift+Tab, Ctrl+h/j/k/l) via the tracked `mods`, so the host never
//    drives the modal — it only forwards keys + `ModifiersChanged`. Keys are routed
//    **field-first**: Esc/Tab/Ctrl-motion are owned by the dialog; every other key is handed
//    to the focused descendant first (so a text `Input` body types + moves its caret), and
//    only an *unconsumed* key falls back to container behaviour (Enter → primary action,
//    arrows → focus motion). ──
impl Dialog {
    /// Move keyboard focus to the next focusable descendant (wraps).
    ///
    /// **Delegated to the composed [`Overlay`](crate::widgets::Overlay), which owns the keyboard.** This
    /// used to drive a `FocusManager` of the dialog's own — a second position over the same panel,
    /// so Tab (the overlay's) and the arrow keys (this one) each thought the keyboard was
    /// somewhere else and one of them was always wrong. One manager, one position.
    fn focus_next(&mut self) {
        self.base.children[0].advance_focus(true);
    }

    /// Move keyboard focus to the previous focusable descendant (wraps). See
    /// [`focus_next`](Dialog::focus_next).
    fn focus_prev(&mut self) {
        self.base.children[0].advance_focus(false);
    }

    /// Fire the **primary** (first) action button — used when Enter is pressed from a field
    /// that didn't consume it (e.g. a focused text input), so Enter submits like clicking OK.
    /// When a button itself is focused, Enter is delivered straight to it (it consumes it), so
    /// this only runs for the non-button-focused case.
    fn activate_primary(&mut self) {
        if !self.has_actions {
            return;
        }
        let panel = self.base.children[0].base_mut().children[0].base_mut();
        // Panel layout is `[title, body?, action-row]`; the action row is the last child, and
        // its first child is the primary (leftmost) button.
        if let Some(row) = panel.children.last_mut()
            && let Some(primary) = row.base_mut().children.first_mut()
        {
            // **Ask it to act; do not pretend to be a keyboard.** This used to dispatch a
            // synthetic `Event::Key { Enter }` and relied on the button claiming raw keys whether
            // or not it was focused.
            primary.activate();
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
        self.base.open.get_untracked()
    }

    /// The panel's laid-out bounds (valid after layout; a zero rect before first layout).
    fn panel_bounds(&self) -> Rectangle {
        self.base
            .children
            .first()
            .and_then(|overlay| overlay.base().children.first())
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

    fn remeasure(&mut self) {
        let max_width = self.panel_max_width.unwrap_or(crate::style::Length::Px(
            self.base.font * self.base.dialog_style.max_width_em,
        ));
        self.base.children[0].base_mut().children[0].base_mut().style.layout.max_width = Some(max_width);
    }

    /// Focusable only while open, so the host's overlay scan routes input here.
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// An open dialog is **modal**: its scrim covers the whole viewport, so every
    /// point is occluded — a host must not synthesize a page-level action (e.g.
    /// open a context menu) anywhere while it is open. (The composed [`Overlay`](crate::widgets::Overlay)
    /// reports the same; this keeps the answer on the widget the host scans first.)
    fn overlay_occludes(&self, _pos: Point) -> bool {
        self.is_open()
    }

    // ── Nothing is forwarded any more ────────────────────────────────────────────────────────
    //
    // This block held five hand-written methods passing `presence` / `show` / `close` /
    // `follow_open` down to the composed `Overlay`. They are gone: arriving and leaving now live
    // on `Base`, so **every** component answers — a `Select`, a `Button`, a pane, a dock — and a
    // composed one is not a special case. `ContextMenu` and `CommandPalette` could never have
    // forwarded anyway, because they are their own panels with no overlay inside to forward to,
    // which is how it became clear that forwarding was the wrong shape rather than the missing
    // piece (AGENTS.md § 0 rule 2: the fix is always centralized).
    //
    // The dialog and the overlay it composes share **one** open flag, handed down once in
    // `Dialog::new`, so the surface still places the keyboard where the dialog said when the flag
    // is raised by any of the three doors.

    // No `paint` override: the default recursion reaches the composed [`Overlay`],
    // which owns the whole layer presentation (scrim, shadow, panel fill, bracket
    // reticle, and the panel's children) inside `with_overlay`.

    /// Capture is now only about the **pointer**: focus trapping, and the modal swallow.
    ///
    /// A dialog used to forward every key, every intent and all typed text to its focused field by
    /// hand — an overlay-aware focus scan running beside the framework's own. It no longer needs
    /// to. Keyboard events are delivered to the focus owner and bubble, and the focus owner inside
    /// an open dialog *is* the field or button this widget's own
    /// [`FocusManager`](crate::focus::FocusManager) focused, so the field gets its text first and
    /// the dialog hears what the field declined on the way back up (in
    /// [`on_event`](Component::on_event)). A nested open overlay — a `Select` in the body — is
    /// focused for the same reason, so it answers `Dismiss` before the dialog does, with nothing
    /// declared.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        let panel_bounds = self.panel_bounds();
        match ev {
            // **Let the press through.** Click-to-focus is the router's, once, for every tree — a
            // press on the panel *body* keeps the focused button where it is, because a press on
            // nothing focusable changes nothing. This method only decides what the scrim does.
            Event::PointerDown(p) => {
                let panel = self.base.children[0].base_mut().children[0].as_mut();
                if panel_bounds.contains(p.pos)
                    || crate::component::overlay_occluded_at(panel, p.pos)
                {
                    Handled::No
                } else if self.dismissible {
                    // Scrim / outside click dismisses only when dismissible.
                    self.fire_dismiss();
                    Handled::Yes
                } else {
                    // Always swallow while open (modal).
                    Handled::Yes
                }
            }
            // Every other pointer kind is routed, not forwarded: the widget under the cursor gets
            // it, and the blocking `Overlay` inside this dialog swallows whatever nothing took —
            // which is what keeps the page behind a modal still.
            _ if ev.pointer().is_some() => Handled::No,
            _ => Handled::No,
        }
    }

    /// **What the focused thing inside the dialog did not want.**
    ///
    /// The bubble phase is where a container's own behaviour belongs, and for a modal it is also
    /// what makes field-first delivery automatic: the walk has already been to the focused field or
    /// button and come back, so an `Input` has had its `Ctrl+h`, a nested `Select` has had its
    /// `Dismiss`, and what arrives here is genuinely the dialog's.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        match ev {
            // A dialog's focus is a **horizontal** button row, so it navigates on
            // `ItemPrevious`/`ItemNext`; `Activate` submits the primary action, `Dismiss` cancels.
            Event::Widget(intent) => match intent {
                WidgetIntent::ItemNext => {
                    self.focus_next();
                    Handled::Yes
                }
                WidgetIntent::ItemPrevious => {
                    self.focus_prev();
                    Handled::Yes
                }
                WidgetIntent::Activate => {
                    self.activate_primary();
                    Handled::Yes
                }
                WidgetIntent::Dismiss => {
                    self.fire_dismiss();
                    Handled::Yes
                }
                _ => Handled::No,
            },
            // Classic, always-on focus traversal: Tab / Shift+Tab move focus within the modal.
            // Universal widget behaviour, not a rebindable `[keys.widgets]` binding.
            Event::Key {
                key: GridKey::Tab,
                pressed: true,
            } => {
                if crate::event::modifiers().shift {
                    self.focus_prev();
                } else {
                    self.focus_next();
                }
                Handled::Yes
            }
            // Any other unconsumed key: `Handled::No`, so the host can resolve it against the
            // configurable `[keys.widgets]` bindings (→ `WidgetIntent`).
            _ => Handled::No,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::PointerButton;
    use crate::widgets::{Button, Label};
    use std::cell::Cell;
    use std::rc::Rc;

    fn open_dialog() -> Dialog {
        Dialog::new("Delete pane?")
            .body(Label::new("This action cannot be undone."))
            .action(Button::new("Cancel"))
            .action(Button::new("Delete"))
            .default_open(true)
    }

    /// An open dialog wired with a dismiss flag, for the Esc / scrim tests.
    fn open_dialog_with_flag() -> (Dialog, Rc<Cell<bool>>) {
        let flag = Rc::new(Cell::new(false));
        let f = flag.clone();
        let d = open_dialog().on_dismiss(move || f.set(true));
        (d, flag)
    }

    #[test]
    fn structure_is_overlay_holding_panel_with_title_body_and_action_row() {
        let d = open_dialog();
        // One child: the composed base Overlay (the blocking layer).
        assert_eq!(d.base().children.len(), 1);
        let overlay = &d.base().children[0];
        // The overlay's single child is the panel: [title, body, action-row].
        assert_eq!(overlay.base().children.len(), 1, "overlay holds the panel");
        let panel = &overlay.base().children[0];
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
            crate::component::dispatch(
                &mut d,
                &Event::Key {
                    key: GridKey::Escape,
                    pressed: true
                }
            ),
            Handled::No,
            "a closed dialog handles nothing",
        );
    }

    #[test]
    fn cancel_nav_fires_dismiss_when_dismissible() {
        // Dismiss arrives as the host-resolved `WidgetIntent::Dismiss` (the app maps `dismiss`,
        // default Esc, to it). The dialog no longer hardcodes the Esc key.
        let (mut d, flag) = open_dialog_with_flag();
        assert_eq!(
            crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::Dismiss)),
            Handled::Yes,
        );
        assert!(flag.get(), "Dismiss fired the dismiss callback");
    }

    /// Buttons in the action row that currently hold focus (drives the focus ring).
    fn focused_buttons(d: &Dialog) -> Vec<usize> {
        let panel = &d.base().children[0].base().children[0];
        let row = &panel.base().children[2];
        row.base()
            .children
            .iter()
            .enumerate()
            .filter(|(_, b)| b.base().is_focused())
            .map(|(i, _)| i)
            .collect()
    }

    /// The whole point of `panel_size`: a sized panel **bounds** its body, so a
    /// `ScrollRegion` inside it actually overflows and scrolls. Unsized, the panel
    /// grows to fit the content and the region never has anything to scroll.
    #[test]
    fn a_sized_panel_bounds_a_scrollable_body() {
        use crate::Length;
        use crate::builders::Parent;
        use crate::widgets::ScrollRegion;

        // 12 rows, far taller than the 200px panel we ask for.
        let long_body = || {
            let mut region = ScrollRegion::new();
            for i in 0..12 {
                region = region.child(Label::new(format!("row {i}")));
            }
            region
        };

        let viewport = Size::new(600.0, 400.0);

        let mut sized = Dialog::new("Long list")
            .panel_size(Length::Px(300.0), Length::Px(200.0))
            .body(long_body())
            .default_open(true);
        crate::LayoutEngine::new().compute(&mut sized, viewport);
        let panel = sized.panel_bounds();
        assert!(
            (panel.size.h - 200.0).abs() < 1.0,
            "panel honours the requested height, got {}",
            panel.size.h
        );
        assert!(
            panel.size.h < viewport.h,
            "a bounded panel is smaller than the viewport, so the body can overflow"
        );

        // Unsized, the same body makes the panel grow instead (nothing to scroll).
        let mut unsized_dialog = Dialog::new("Long list")
            .body(long_body())
            .default_open(true);
        crate::LayoutEngine::new().compute(&mut unsized_dialog, viewport);
        assert!(
            unsized_dialog.panel_bounds().size.h > panel.size.h,
            "without panel_size the panel hugs the tall content"
        );
    }

    #[test]
    fn clicking_panel_body_keeps_button_focus() {
        // Regression: a press on the panel *body* (not a button) must NOT clear the
        // focused button's ring — focus is trapped inside the modal. Previously the
        // panel dispatch cleared focus on any click that missed every focusable.
        let mut d = open_dialog();
        crate::LayoutEngine::new().compute(&mut d, Size::new(600.0, 400.0));
        // Move focus by nav so a button shows the focus ring.
        let _ = crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::ItemNext));
        let before = focused_buttons(&d);
        assert!(!before.is_empty(), "a button is focused after ItemNext");

        // Click the panel body: inside the panel bounds but on the title band (no button).
        let panel = d.panel_bounds();
        let body = Point::new(panel.loc.x + panel.size.w * 0.5, panel.loc.y + 2.0);
        assert!(panel.contains(body), "test point is inside the panel body");
        let _ =
            crate::component::dispatch(&mut d, &Event::pointer_pressed(body, PointerButton::Left));

        assert_eq!(
            focused_buttons(&d),
            before,
            "clicking the panel body keeps the focused button (trapped focus)",
        );
    }

    #[test]
    fn cancel_dismisses_even_when_forced_but_scrim_does_not() {
        // `WidgetIntent::Dismiss` is a universal modal cancel: it fires `on_dismiss` even on a
        // `dismissible(false)` (forced-choice) dialog. Only the scrim / outside-click is gated by
        // `dismissible`.
        let flag = Rc::new(Cell::new(false));
        let f = flag.clone();
        let mut d = open_dialog()
            .dismissible(false)
            .on_dismiss(move || f.set(true));
        assert_eq!(
            crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::Dismiss)),
            Handled::Yes,
        );
        assert!(flag.get(), "Dismiss dismisses even a forced dialog");

        // A scrim click (press outside the panel) on a forced dialog must NOT dismiss.
        flag.set(false);
        let _ = crate::component::dispatch(
            &mut d,
            &Event::pointer_pressed(Point::new(-100.0, -100.0), PointerButton::Left),
        );
        assert!(!flag.get(), "forced dialog ignores the scrim/outside click");
    }

    #[test]
    fn dialog_nav_moves_focus() {
        // Focus nav arrives as the semantic horizontal `Item*` intents (the host maps the
        // configurable `item_next`/`item_previous` bindings). Each is consumed and lands focus.
        for intent in [WidgetIntent::ItemNext, WidgetIntent::ItemPrevious] {
            let mut d = open_dialog();
            assert_eq!(
                crate::component::dispatch(&mut d, &Event::Widget(intent)),
                Handled::Yes
            );
            assert!(
                !focused_buttons(&d).is_empty(),
                "{intent:?} focuses a button"
            );
        }
    }

    #[test]
    fn text_input_body_types_and_submit_nav_fires_primary() {
        use crate::widgets::Input;
        let fired = Rc::new(Cell::new(false));
        let f = fired.clone();
        // A form dialog: an Input body + OK (primary) / Cancel. `open` focuses the Input first.
        let mut d = Dialog::new("Rename pane")
            .body(Input::new().value("term"))
            .action(Button::new("OK").on_click(move || f.set(true)))
            .action(Button::new("Cancel"))
            .default_open(true);
        // Typed text is delivered field-first to (and consumed by) the focused input. Text, not a
        // key: `Event::TextInput` is what the user actually committed.
        assert_eq!(
            crate::component::dispatch(&mut d, &Event::TextInput("x".to_string())),
            Handled::Yes,
            "the focused input receives typed characters",
        );
        // `WidgetIntent::Activate` (host maps `activate`, default Enter) fires the PRIMARY action.
        assert!(!fired.get());
        let _ = crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::Activate));
        assert!(fired.get(), "Activate fires the primary action (OK)");
    }

    #[test]
    fn input_edit_reaches_focused_field_nav_moves_to_button() {
        use crate::widgets::Input;
        // Field-first + host-resolved editing: an `Edit*` intent is forwarded to the focused
        // input (consumed there, focus stays on it), while an `Item*` intent moves focus.
        let mut d = Dialog::new("Rename")
            .body(Input::new().value("ab"))
            .action(Button::new("OK"))
            .action(Button::new("Cancel"))
            .default_open(true);
        // Editing shortcut → forwarded to the input; it is consumed and focus stays on the field.
        assert_eq!(
            crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::EditDeleteBack)),
            Handled::Yes,
            "EditDeleteBack reaches the focused input",
        );
        assert!(
            focused_buttons(&d).is_empty(),
            "editing keeps focus in the input"
        );
        // Nav moves focus off the input onto a button.
        let _ = crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::ItemNext));
        assert!(
            !focused_buttons(&d).is_empty(),
            "ItemNext navigates to a button"
        );
    }

    // ── A dialog answers as a surface ──────────────────────────────────────

    /// **A host can raise a dialog, and until now it could not.** ⚠️ Ran red against its own bug.
    ///
    /// A `Dialog` is composed on an `Overlay` but forwarded only its keyboard, so asking the
    /// dialog itself to open reached the `Component` trait's empty default and did nothing at all.
    /// The single thing that had ever raised one was `default_open(true)` at construction — which
    /// is why a dialog could not be shown by name, and why a handle pointing at one moved a flag
    /// no surface was reading.
    #[test]
    fn asking_a_dialog_to_show_actually_opens_it() {
        let mut d = Dialog::new("Delete pane?")
            .body(Label::new("This action cannot be undone."))
            .action(Button::new("Cancel"));
        assert!(!d.is_open(), "built closed, as every surface is");

        Component::show(&mut d);
        assert!(
            d.is_open(),
            "the dialog forwards opening to the overlay it is built on, so a host that holds a \
             surface can raise it without knowing what kind it is",
        );

        Component::close(&mut d);
        assert!(!d.is_open(), "and the same for dismissing it");
    }

    /// **A dialog reports its own arrival and exit**, so a host can tell a surface that is going
    /// away from one that has gone.
    ///
    /// It answered `None` before, which reads as "not a surface at all": a dismissed dialog
    /// counted as finished the instant it was asked to leave, rather than when its gesture had
    /// played out.
    #[test]
    fn a_dialog_reports_whether_it_is_up() {
        use crate::animation::Presence;

        let mut d = open_dialog();
        assert!(
            d.presence().is_some_and(Presence::is_open),
            "an open dialog says it is up",
        );

        Component::close(&mut d);
        assert!(
            d.presence().is_some_and(|p| !p.is_open()),
            "and a dismissed one says it is not",
        );
    }

    /// **A dialog raised by its handle starts on the control it named.**
    ///
    /// The same rule the overlay guards prove, asserted through the composed widget, because that
    /// is the shape a caller actually holds — and because forwarding a subset of a surface's
    /// behaviour is what broke the keyboard the last time it was done by halves.
    #[test]
    fn a_dialog_raised_by_its_handle_starts_where_it_said() {
        let mut d = Dialog::new("Delete pane?")
            .body(Label::new("This action cannot be undone."))
            .action(Button::new("Cancel"))
            .action(Button::new("Delete"))
            .default_action("Delete");

        let handle = d.handle();
        let opener = move || handle.show();
        opener();
        d.tick(0.016);

        fn focused(n: &dyn Component, out: &mut Vec<String>) {
            if n.base().is_focused()
                && let Some(name) = n.text_summary()
            {
                out.push(name);
            }
            for c in &n.base().children {
                focused(c.as_ref(), out);
            }
        }
        let mut names = Vec::new();
        focused(&d, &mut names);
        assert!(
            names.iter().any(|n| n == "Delete"),
            "opened from a closure holding nothing but the handle, and the keyboard landed on the \
             control the dialog named: {names:?}",
        );
    }
}

#[cfg(test)]
mod tab_repro {
    use super::*;
    use crate::component::GridKey;
    use crate::widgets::{Button, Label};

    /// **The first Tab moves the keyboard**.
    ///
    /// It did not. Opening a dialog quietly focused its first button through a `FocusManager` the
    /// **dialog** owned, while Tab was answered by the one the composed **overlay** owns — two
    /// positions over one panel. The first press moved the overlay's manager to *its* first
    /// control, which was the button the keyboard was already on, so nothing appeared to happen
    /// and only the second press moved.
    ///
    /// One manager now, on the overlay, which is what holds the keyboard. The dialog keeps none
    /// and delegates all four operations — opening, Tab, arrow motion, and a click inside the
    /// panel. Collapsing only *one* of them is what broke the arrow-key traversal on the first
    /// attempt: the halves have to move together or they disagree in a new place instead.
    #[test]
    fn the_first_tab_in_a_dialog_moves_off_the_default_button() {
        let mut d = Dialog::new("Close pane?")
            .body(Label::new("This action cannot be undone."))
            .action(Button::new("Cancel"))
            .action(Button::new("Close"))
            .default_open(true);
        crate::component::dispatch(
            &mut d,
            &Event::Key {
                key: GridKey::Tab,
                pressed: true,
            },
        );

        fn focused(n: &dyn Component, out: &mut Vec<String>) {
            if n.base().is_focused()
                && let Some(name) = n.text_summary()
            {
                out.push(name);
            }
            for c in &n.base().children {
                focused(c.as_ref(), out);
            }
        }
        let mut names = Vec::new();
        focused(&d, &mut names);

        assert!(
            names.iter().any(|n| n == "Close"),
            "one Tab moved off the button the dialog opened on, got {names:?}",
        );
        assert!(
            !names.iter().any(|n| n == "Cancel"),
            "and left it, rather than lighting both: {names:?}",
        );
    }
}
