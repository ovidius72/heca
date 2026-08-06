//! [`Dialog`] — a centered overlay **panel that holds real child components**.
//!
//! `Dialog` is a true container: it lays out a centered panel over a dimming scrim and paints
//! its **children** (a title, an arbitrary `body`, and a row of real action
//! [`Button`](super::Button)s). Because the buttons are real components, they participate in the
//! universal hint picker, standard focus traversal, and pointer routing for free (a hand-drawn
//! panel that painted its own buttons could do none of those).
//!
//! It uses the same overlay contract as [`Select`](super::Select): it reports
//! [`overlay_active`](Component::overlay_active) + [`focusable`](Component::focusable) while
//! open, so the host routes input here first.
//!
//! **Nav keys are host-configured** (`widget-keys-config`) except the universal focus primitive.
//! **Tab / Shift+Tab always move focus** within the modal — classic, always-on behaviour handled
//! by the embedded [`FocusManager`](crate::focus::FocusManager), not a rebindable binding. Beyond
//! that the dialog carries no literal keys: its button row is a **horizontal** focus strip, so
//! configurable traversal arrives as the semantic [`Event::Widget`] intents
//! `ItemPrevious`/`ItemNext`; `Activate` submits the primary action and `Dismiss` cancels. The
//! host resolves these from the configurable `[keys.widgets]` `item_previous` / `item_next` /
//! `activate` / `dismiss` bindings (defaults: ←/→, vim `Ctrl+h`/`Ctrl+l`, Enter, Esc). The
//! `Edit*` intents (and any vertical `Menu*`) and a raw [`Event::Key`] are delivered
//! **field-first**: the focused
//! widget (e.g. an [`Input`](super::Input)) consumes its own typing/editing first via the
//! embedded [`FocusManager`](crate::focus::FocusManager); only unconsumed intents move focus.
//! `Activate` fires the primary action (its `on_click`); `Dismiss` (or a
//! scrim click) fires the [`on_dismiss`](Dialog::on_dismiss) callback (there are no result
//! closures baked in; a button's own `on_click` carries the action, and dismissal is a callback
//! the host points at its own overlay-close path).
//!
//! **Presentation is composed, not owned:** the `Dialog` mounts a base
//! [`Overlay`](super::Overlay) (blocking layer — scrim, drop shadow, panel fill, bracket
//! reticle, viewport centering) and puts its padded `[title, body?, action-row?]` panel inside
//! it. The `Overlay` owns the layer chrome and geometry; the `Dialog` owns the content and the
//! behaviour (focus trap, keyboard, dismissal policy) and intercepts events before the
//! `Overlay`'s standalone handling would run. Centering is real layout (the overlay fills the
//! viewport and centers the panel by taffy), so every descendant gets true bounds (which the
//! hint picker and pointer hit-testing need).

use crate::builders::{LayoutExt, Parent};
use crate::component::{
    Base, Component, Event, GridKey, Handled, Modifiers, WidgetIntent,
};
use crate::focus::FocusManager;
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use crate::style::{Justify, Length, Spacing};
use crate::widgets::{Flex, Label, Overlay};
use heca_core::layout::{Point, Rectangle, Size};

/// Panel inner padding.
const PAD: f32 = 18.0;
/// Gap between the title, body, and the action row.
const GAP: f32 = 14.0;
/// Gap between adjacent action buttons.
const BTN_GAP: f32 = 10.0;

/// A centered overlay panel over a scrim, holding real child components.
///
/// Structure: the `Dialog` base is a full-viewport passthrough whose single child is a
/// **blocking [`Overlay`](super::Overlay)** (scrim + chrome + centering), whose single child
/// is the **panel** — a padded column of `[title, body?, action-row?]`. The action row is a
/// right-aligned [`Flex`] row of the caller's [`Button`](super::Button)s.
pub struct Dialog {
    base: Base,
    /// Shared with the composed [`Overlay`] (it is the overlay's own signal).
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
    /// Live modifier state (from the broadcast [`Event::ModifiersChanged`]) — needed so the
    /// widget can tell Tab from Shift+Tab for its classic, always-on focus traversal.
    mods: Modifiers,
    /// Whether an action row exists yet (created lazily on the first [`action`](Dialog::action)).
    has_actions: bool,
}

#[heca_grid_ui_macros::props]
impl Dialog {
    /// A new (closed) dialog titled `title`. Add content with [`body`](Dialog::body) and
    /// buttons with [`action`](Dialog::action), in that order.
    pub fn new(title: impl Into<String>) -> Self {
        // Panel: a padded column holding the title, then body + actions as they're added.
        let panel = Flex::column()
            .padding(PAD)
            .gap(GAP)
            .child(Label::new(title));

        // The base Overlay owns the layer presentation: blocking (scrim + swallow),
        // viewport centering, drop shadow, panel fill, bracket reticle. Its open
        // signal IS the dialog's open signal.
        let overlay = Overlay::new().blocking(true).panel(panel);
        let open = overlay.open_signal();

        // Root: a full-size passthrough so the overlay child fills the viewport.
        let mut base = Base::new();
        base.style.layout.width = Length::Pct(1.0);
        base.style.layout.height = Length::Pct(1.0);
        base.children.push(Box::new(overlay));

        Self {
            base,
            open,
            dismissible: true,
            focus: FocusManager::new(),
            on_dismiss: None,
            mods: Modifiers::default(),
            has_actions: false,
        }
    }

    /// Give the dialog panel an explicit size instead of letting it hug its content.
    ///
    /// The reason you usually want it: **a [`ScrollRegion`](super::ScrollRegion) only
    /// scrolls when its parent bounds it.** A panel that sizes to its content just grows
    /// with a long [`body`](Dialog::body), so nothing overflows and no scrollbar appears.
    /// Give the panel a height and the body scrolls inside it — while the title and the
    /// action row stay put, because only the body is inside the region:
    ///
    /// ```ignore
    /// Dialog::new("Pick a container")
    ///     .panel_size(Length::Pct(0.5), Length::Pct(0.6))   // 50% × 60% of the VIEWPORT
    ///     .body(ScrollRegion::new().child(long_list))       // only the body scrolls
    ///     .action(Button::new("Cancel"))
    /// ```
    ///
    /// `Length::Auto` on an axis keeps the hug-content behaviour. A [`Pct`](Length::Pct)
    /// resolves against the **viewport** (the composed [`Overlay`](super::Overlay) fills it).
    #[heca_grid_ui_macros::host_only("takes more than one value, which a single property cannot carry")]
    pub fn panel_size(mut self, width: Length, height: Length) -> Self {
        let style = &mut self.panel_mut().style.layout;
        style.width = width;
        style.height = height;
        self
    }

    /// Set the dialog **body** — an arbitrary component (a message label, a form, a table…),
    /// inserted between the title and the action row. Call before [`action`](Dialog::action).
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn body(mut self, body: impl Component + 'static) -> Self {
        self.panel_mut().children.push(Box::new(body));
        self.fit_body();
        self
    }

    /// Like [`body`](Dialog::body) but takes an already-boxed component — for a body produced
    /// by a mapper that returns `Box<dyn Component>` (e.g. `heca`'s `realize(ViewNode)`), which
    /// can't be passed to `body` because `Box<dyn Component>` is not itself `Component`.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn body_boxed(mut self, body: Box<dyn Component>) -> Self {
        self.panel_mut().children.push(body);
        self.fit_body();
        self
    }

    /// Make the body behave the way a dialog body always should, so no caller has
    /// to remember it: **fill the panel's width** (instead of hugging its content
    /// and sitting to the left) and **take the space left between the title and the
    /// action row** (instead of the row floating up under a short body).
    ///
    /// The second part is what makes a scrollable body work: with a bounded panel
    /// ([`panel_size`](Dialog::panel_size)) the body gets the leftover height, so a
    /// [`ScrollRegion`](super::ScrollRegion) inside it has a real viewport to
    /// overflow — and the title and buttons stay put because only the body flexes.
    /// With an unsized panel there is no leftover space, so this changes nothing.
    ///
    /// It also gives the body a default **gap between its children** so a dialog's
    /// fields breathe whether or not the body scrolls — the same spacing a
    /// [`ScrollRegion`](super::ScrollRegion) body already applies to itself, now
    /// applied to a plain container body too, so the modal is consistent either
    /// way. (A label + its control should be grouped into one tight unit, e.g. a
    /// `Field`, so this larger gap falls *between* fields, never between a label
    /// and its own input.)
    ///
    /// Only defaults are filled in: an explicit width / grow / gap the caller set
    /// is respected.
    fn fit_body(&mut self) {
        let idx = self.panel_mut().children.len() - 1;
        let style = &mut self.panel_mut().children[idx].base_mut().style.layout;
        if style.width == Length::Auto {
            style.width = Length::Pct(1.0);
        }
        if style.flex_grow == 0.0 {
            style.flex_grow = 1.0;
        }
        if style.gap == 0.0 && style.gap_spacing.is_none() {
            style.gap_spacing = Some(Spacing::Md);
        }
    }

    /// Append an action **button** (a real [`Button`](super::Button) the caller has already
    /// wired with its `on_click` + hint target). Buttons live in a right-aligned row along
    /// the panel bottom, in call order.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
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
    #[heca_grid_ui_macros::prop]
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Set the callback fired when Esc or a scrim click requests dismissal (respecting
    /// [`dismissible`](Dialog::dismissible)). The host wires this to its overlay-close path.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    /// Set the initial open state (focusing the first focusable — the safe default when the
    /// caller orders `[Cancel, …, Confirm]`).
    #[heca_grid_ui_macros::prop]
    pub fn open(mut self, open: bool) -> Self {
        self.open.set(open);
        if open {
            // Focus the safe-default (first) button so Enter works — but WITHOUT the ring; it
            // appears only once the user navigates by keyboard (focus-visible).
            let panel = self.base.children[0].base_mut().children[0].as_mut();
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
        let panel = self.base.children[0].base_mut().children[0].as_mut();
        self.focus.advance(panel, true);
    }

    /// Move keyboard focus to the previous focusable descendant (wraps).
    fn focus_prev(&mut self) {
        let panel = self.base.children[0].base_mut().children[0].as_mut();
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
        self.open.get_untracked()
    }

    /// The panel container (the composed [`Overlay`]'s single child), mutably.
    fn panel_mut(&mut self) -> &mut Base {
        self.base.children[0].base_mut().children[0].base_mut()
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

    /// Focusable only while open, so the host's overlay scan routes input here.
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// An open dialog is **modal**: its scrim covers the whole viewport, so every
    /// point is occluded — a host must not synthesize a page-level action (e.g.
    /// open a context menu) anywhere while it is open. (The composed [`Overlay`]
    /// reports the same; this keeps the answer on the widget the host scans first.)
    fn overlay_occludes(&self, _pos: Point) -> bool {
        self.is_open()
    }

    // No `paint` override: the default recursion reaches the composed [`Overlay`],
    // which owns the whole layer presentation (scrim, shadow, panel fill, bracket
    // reticle, and the panel's children) inside `with_overlay`.

    /// Owns its walk. A modal routes through an **overlay-aware focus scan**
    /// (`dispatch_trapped` / `offer_to_overlay`), not a child walk: a nested open overlay — a
    /// `Select` dropdown in the body — gets input first even when the pointer is outside the
    /// panel, because its list can extend past the panel edge. Focus is also trapped, so a press
    /// on empty panel space must keep the focused button's ring rather than clear it.
    ///
    /// The per-kind arms below are the reason this widget is held to
    /// `tests/pointer_delivery.rs`: the release and the wheel arms exist because a `ScrollRegion`
    /// in a dialog body was found stuck and unscrollable, and nothing but that test stops the next
    /// kind going missing the same way.
    fn routes_own_subtree(&self) -> bool {
        true
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        let panel_bounds = self.panel_bounds();
        match ev {
            // **Focus, then let the press through.** Capture runs before the target, so the
            // click-to-focus decision is made here and the router still carries the press to
            // whatever is under the cursor — this method no longer delivers anything itself.
            // Trapped focus: a press on the panel *body* keeps the focused button's ring rather
            // than clearing it, because focus is trapped inside a modal.
            Event::PointerDown(p) => {
                let panel = self.base.children[0].base_mut().children[0].as_mut();
                if panel_bounds.contains(p.pos)
                    || crate::component::overlay_occluded_at(panel, p.pos)
                {
                    self.focus.focus_at_trapped(panel, p.pos);
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
            // Track modifiers (for Shift+Tab) and forward the broadcast to the panel so a focused
            // field's own modifier-aware editing (word/line motion) sees it. Not consumed.
            Event::ModifiersChanged(m) => {
                self.mods = *m;
                let panel = self.base.children[0].base_mut().children[0].as_mut();
                let _ = crate::component::dispatch(panel, ev);
                Handled::No
            }
            // Host-resolved intents (`[keys.widgets]` → `WidgetIntent`). A dialog's focus is a
            // **horizontal** button row, so it navigates on `ItemPrevious`/`ItemNext`; `Activate`
            // submits the primary action and `Dismiss` cancels. Every other intent (the `Edit*`
            // shortcuts, and any vertical `Menu*`) is forwarded **field-first** to the focused
            // widget — so a focused `Input` gets its `Ctrl+h` delete while a focused button lets
            // the nav overload through. The host delivers `Edit*` before `Item*`, so a focused
            // field consumes its edit before the dialog would navigate.
            Event::Widget(intent) => {
                // A NESTED open overlay (a Select dropdown in the body) owns the semantic
                // intents first: `Dismiss` closes IT (not the dialog), `Activate` commits
                // ITS row, `Menu*` move its cursor. Only unconsumed intents fall through
                // to the dialog's own handling.
                let panel = self.base.children[0].base_mut().children[0].as_mut();
                if self.focus.offer_to_overlay(panel, ev) == Handled::Yes {
                    return Handled::Yes;
                }
                match intent {
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
                    _ => {
                        let panel = self.base.children[0].base_mut().children[0].as_mut();
                        self.focus.deliver_event(panel, ev)
                    }
                }
            }
            // Typed text goes to whoever has the keyboard, like a key — a dialog has no use for
            // text of its own.
            Event::TextInput(_) => {
                let panel = self.base.children[0].base_mut().children[0].as_mut();
                self.focus.deliver_event(panel, ev)
            }
            // **Field-first**: hand the raw key to the focused widget so it keeps ALL its native
            // behaviour — an `Input`'s typing, caret motion, Backspace/Delete/Home/End. (A nested
            // open overlay is usually the focused widget too — clicking its trigger focused it —
            // so its keys arrive through the same field-first delivery.)
            Event::Key { key, pressed: true } => {
                let panel = self.base.children[0].base_mut().children[0].as_mut();
                if self.focus.deliver_key(panel, *key) == Handled::Yes {
                    return Handled::Yes;
                }
                // Classic, always-on focus traversal: Tab / Shift+Tab move focus within the modal.
                // This is universal widget behaviour, not a rebindable `[keys.widgets]` binding.
                if matches!(key, GridKey::Tab) {
                    if self.mods.shift {
                        self.focus_prev();
                    } else {
                        self.focus_next();
                    }
                    return Handled::Yes;
                }
                // Any other unconsumed key: report `Handled::No` so the host can resolve it against
                // the configurable `[keys.widgets]` bindings (→ `WidgetIntent`).
                Handled::No
            }
            _ => Handled::No,
        }
    }
}

impl LayoutExt for Dialog {}

#[cfg(test)]
mod tests {
    use crate::event::PointerButton;
    use super::*;
    use crate::widgets::{Button, Label};
    use std::cell::Cell;
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
            crate::component::dispatch(&mut d, &Event::Key { key: GridKey::Escape, pressed: true }),
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
        use crate::reactive::SignalGet;
        let panel = &d.base().children[0].base().children[0];
        let row = &panel.base().children[2];
        row.base()
            .children
            .iter()
            .enumerate()
            .filter(|(_, b)| b.base().focused.get_untracked())
            .map(|(i, _)| i)
            .collect()
    }

    /// The whole point of `panel_size`: a sized panel **bounds** its body, so a
    /// `ScrollRegion` inside it actually overflows and scrolls. Unsized, the panel
    /// grows to fit the content and the region never has anything to scroll.
    #[test]
    fn a_sized_panel_bounds_a_scrollable_body() {
        use crate::widgets::ScrollRegion;
        use crate::Length;

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
            .open(true);
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
        let mut unsized_dialog = Dialog::new("Long list").body(long_body()).open(true);
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
        let _ = crate::component::dispatch(&mut d, &Event::pointer_pressed(body, PointerButton::Left));

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
        let mut d = open_dialog().dismissible(false).on_dismiss(move || f.set(true));
        assert_eq!(
            crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::Dismiss)),
            Handled::Yes,
        );
        assert!(flag.get(), "Dismiss dismisses even a forced dialog");

        // A scrim click (press outside the panel) on a forced dialog must NOT dismiss.
        flag.set(false);
        let _ = crate::component::dispatch(&mut d, &Event::pointer_pressed(Point::new(-100.0, -100.0), PointerButton::Left));
        assert!(!flag.get(), "forced dialog ignores the scrim/outside click");
    }

    #[test]
    fn dialog_nav_moves_focus() {
        // Focus nav arrives as the semantic horizontal `Item*` intents (the host maps the
        // configurable `item_next`/`item_previous` bindings). Each is consumed and lands focus.
        for intent in [WidgetIntent::ItemNext, WidgetIntent::ItemPrevious] {
            let mut d = open_dialog();
            assert_eq!(crate::component::dispatch(&mut d, &Event::Widget(intent)), Handled::Yes);
            assert!(!focused_buttons(&d).is_empty(), "{intent:?} focuses a button");
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
            .open(true);
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
            .open(true);
        // Editing shortcut → forwarded to the input; it is consumed and focus stays on the field.
        assert_eq!(
            crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::EditDeleteBack)),
            Handled::Yes,
            "EditDeleteBack reaches the focused input",
        );
        assert!(focused_buttons(&d).is_empty(), "editing keeps focus in the input");
        // Nav moves focus off the input onto a button.
        let _ = crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::ItemNext));
        assert!(!focused_buttons(&d).is_empty(), "ItemNext navigates to a button");
    }
}
