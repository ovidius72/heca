//! [`Dialog`] — a centered overlay **panel that holds real child components**.
//!
//! `Dialog` is a true container: it lays out a centered panel over a dimming scrim and paints
//! its **children** (a title, an arbitrary `body`, and a row of real action
//! [`Button`](super::Button)s). Because the buttons are real components, they participate in the
//! universal hint picker, standard focus traversal, and pointer routing for free (a hand-drawn
//! panel that painted its own buttons could do none of those).
//!
//! It uses the same overlay contract as [`Select`](super::Select): it reports
//! [`overlay_active`](crate::component::Component::overlay_active) + [`focusable`](crate::component::Component::focusable) while
//! open, so the host routes input here first.
//!
//! **Composed, not hand-run.** The `Dialog` is a blocking [`Overlay`](crate::widgets::Overlay)
//! over a padded panel of `[title, body?, action-row?]`, built the first time it is needed — the
//! first layout, event or question about the keyboard — once every builder has run, and never
//! reached into after. The overlay owns the layer and the keyboard: the scrim, the chrome, the
//! centering, the panel kept inside the window, Tab, the arrows across the buttons
//! ([`item_keys`](crate::widgets::Overlay::item_keys)), Esc (the dialog's
//! [`on_dismiss`](Dialog::on_dismiss)) and a press outside (the same, when
//! [`dismissible`](Dialog::dismissible)). The panel adds what only a dialog knows: Enter, when
//! nothing focused took it, presses the primary (first) action. The keys are the configurable
//! `[keys.widgets]` `item_previous` / `item_next` / `activate` / `dismiss` (defaults ←/→,
//! `Ctrl+h`/`Ctrl+l`, Enter, Esc), delivered field-first: a focused [`Input`](super::Input) types
//! and moves its caret before the dialog hears what it declined.

use crate::builders::{IntoChildren, LayoutExt, Parent};
use crate::component::Base;
use crate::style::Spacing;

mod builders;
mod component;

// ── The dialog panel recipe ───────────────────────────────────────────────────────────────
//
// **Theme steps, and public, because a dialog is not the only thing shaped like one.** These were
// three private `f32` pixel constants living in this file, which had two consequences: they did not
// follow the font or the theme, and **nothing outside could match them** — so a surface composed
// from `Overlay` + `Surface` + `Button`s, which is what a plugin writes, produced a dialog that
// looked nothing like heca's own. Buttons flush together, no air between the body and the actions
//
// One vocabulary, read by both, so the two cannot drift: change a step here and every dialog-shaped
// surface follows, whoever built it.

/// Inner padding of a dialog panel.
pub const DIALOG_PAD: Spacing = Spacing::Lg;
/// Space between a dialog's title, its body, and its action row.
pub const DIALOG_GAP: Spacing = Spacing::Md;
/// Space between adjacent action buttons.
pub const DIALOG_BTN_GAP: Spacing = Spacing::Sm;

const TITLE_AREA: &str = "dialog-title";
const BODY_AREA: &str = "dialog-body";
const ACTIONS_AREA: &str = "dialog-actions";
const LAYOUT_AREA: &str = "dialog-layout";

/// A centered overlay panel over a scrim, holding real child components.
///
/// Structure: a full-viewport passthrough whose one child — built the first time it is needed,
/// once every builder has run — is a **blocking [`Overlay`](crate::widgets::Overlay)** (scrim,
/// chrome, centering, the keyboard: Tab, the arrows across the buttons, Esc, a press outside) over
/// the **panel**: a padded box holding a Grid of named title, body and action areas. The action row
/// is a right-aligned [`Flex`](crate::widgets::Flex) row of the caller's
/// [`Button`](super::Button)s. Nothing is reached into after it is built.
pub struct Dialog {
    base: Base,
    /// Caller-owned panel style (`panel_size`, `panel_max_width`), handed to the panel when built.
    authored_style: crate::style::Style,
    /// What the panel is built from.
    spec: std::rc::Rc<std::cell::RefCell<builders::Spec>>,
    /// When `false`, a press outside is swallowed but does not dismiss — a forced-decision dialog
    /// (the user must pick a button; Esc still cancels). Default `true`.
    dismissible: std::rc::Rc<std::cell::Cell<bool>>,
    /// Fired when Esc or (if `dismissible`) a press outside requests dismissal. The host points
    /// this at its overlay-close path (e.g. emit `CloseOverlay`).
    on_dismiss: builders::DismissFn,
}

/// The layout builders size the **panel** (`width`, `height`, `max_width`, …) — except the size
/// variant, which stays on the dialog itself so the overlay and the panel inherit it, as every
/// widget's children inherit theirs.
impl LayoutExt for Dialog {
    fn size(mut self, size: crate::style::WidgetSize) -> Self {
        self.base.style.layout.set_size(size);
        self
    }
}

impl Parent for Dialog {
    fn child(self, children: impl IntoChildren) -> Self {
        Dialog::child(self, children)
    }
}
