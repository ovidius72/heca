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
        // The first declared action is primary; body-builder order cannot change its row.
        if let Some(row) = self.part_mut(ACTIONS_AREA)
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

    /// Resolve caller-owned panel style without replacing its original dimensions/limits.
    fn constrain_panel_width(&mut self) -> bool {
        let overlay = self.base.children[0].base();
        let panel = overlay.children[0].base();
        let font = panel.font;
        let available_w = (overlay.bounds.size.w as f32
            - overlay.style.layout.pad_left(overlay.font)
            - overlay.style.layout.pad_right(overlay.font)).max(0.0);
        let available_h = (overlay.bounds.size.h as f32
            - overlay.style.layout.pad_top(overlay.font)
            - overlay.style.layout.pad_bottom(overlay.font)).max(0.0);
        let resolve = |length, available| match length {
            crate::Length::Em(em) => em * font,
            crate::Length::Px(px) => px,
            crate::Length::Percent(fraction) => available * fraction,
            crate::Length::Auto => available,
        };
        let mut style = self.authored_style;
        if !style.layout.size_explicit { style.layout.size = panel.style.layout.size; }
        let requested = style.layout.max_width.unwrap_or(crate::Length::Em(self.base.dialog_style.max_width_em));
        let maximum = resolve(requested, available_w);
        let maximum = if overlay.bounds.size.w > 0.0 { maximum.min(available_w) } else { maximum };
        let minimum = style.layout.min_width.unwrap_or(if style.layout.width == crate::Length::Auto {
            Self::BASELINE_WIDTH
        } else { crate::Length::Auto });
        style.layout.min_width = Some(crate::Length::Px(resolve(minimum, available_w).min(maximum)));
        if minimum == crate::Length::Auto { style.layout.min_width = Some(crate::Length::Auto); }
        style.layout.max_width = Some(crate::Length::Px(maximum));
        if overlay.bounds.size.h > 0.0 {
            let limit = style.layout.max_height.map_or(available_h, |height| resolve(height, available_h).min(available_h));
            style.layout.max_height = Some(crate::Length::Px(limit));
            if let Some(minimum) = style.layout.min_height {
                style.layout.min_height = Some(crate::Length::Px(resolve(minimum, available_h).min(limit)));
            }
        } else {
            style.layout.max_height.get_or_insert(crate::Length::Percent(1.0));
        }
        let panel = &mut self.base.children[0].base_mut().children[0].base_mut().style;
        let changed = *panel != style;
        *panel = style;
        changed
    }
}

impl Component for Dialog {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn authored_style(&self) -> &crate::style::Style { &self.authored_style }
    fn authored_style_mut(&mut self) -> &mut crate::style::Style { &mut self.authored_style }

    fn remeasure(&mut self) {
        self.constrain_panel_width();
    }

    fn on_layout(&mut self) {
        if self.constrain_panel_width() {
            self.base.mark_needs_layout();
        }
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
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tab_repro.rs"]
mod tab_repro;

#[cfg(test)]
#[path = "sizing_tests.rs"]
mod sizing_tests;
