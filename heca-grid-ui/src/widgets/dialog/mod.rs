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
//! **Nav keys are host-configured** (`widget-keys-config`) except the universal focus primitive.
//! **Tab / Shift+Tab always move focus** within the modal — classic, always-on behaviour handled
//! by the embedded [`FocusManager`](crate::focus::FocusManager), not a rebindable binding. Beyond
//! that the dialog carries no literal keys: its button row is a **horizontal** focus strip, so
//! configurable traversal arrives as the semantic [`Event::Widget`](crate::component::Event::Widget) intents
//! `ItemPrevious`/`ItemNext`; `Activate` submits the primary action and `Dismiss` cancels. The
//! host resolves these from the configurable `[keys.widgets]` `item_previous` / `item_next` /
//! `activate` / `dismiss` bindings (defaults: ←/→, vim `Ctrl+h`/`Ctrl+l`, Enter, Esc). The
//! `Edit*` intents (and any vertical `Menu*`) and a raw [`Event::Key`](crate::component::Event::Key) are delivered
//! **field-first**: the focused
//! widget (e.g. an [`Input`](super::Input)) consumes its own typing/editing first via the
//! embedded [`FocusManager`](crate::focus::FocusManager); only unconsumed intents move focus.
//! `Activate` fires the primary action (its `on_click`); `Dismiss` (or a
//! scrim click) fires the [`on_dismiss`](Dialog::on_dismiss) callback (there are no result
//! closures baked in; a button's own `on_click` carries the action, and dismissal is a callback
//! the host points at its own overlay-close path).
//!
//! **Presentation is composed, not owned:** the `Dialog` mounts a base
//! [`Overlay`](crate::widgets::Overlay) (blocking layer — scrim, drop shadow, panel fill, bracket
//! reticle, viewport centering) and puts its padded `[title, body?, action-row?]` panel inside
//! it. The `Overlay` owns the layer chrome and geometry; the `Dialog` owns the content and the
//! behaviour (focus trap, keyboard, dismissal policy) and intercepts events before the
//! `Overlay`'s standalone handling would run. Centering is real layout (the overlay fills the
//! viewport and centers the panel by taffy), so every descendant gets true bounds (which the
//! hint picker and pointer hit-testing need).

use crate::builders::LayoutExt;
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

/// A centered overlay panel over a scrim, holding real child components.
///
/// Structure: the `Dialog` base is a full-viewport passthrough whose single child is a
/// **blocking [`Overlay`](crate::widgets::Overlay)** (scrim + chrome + centering), whose single child
/// is the **panel** — a padded column of `[title, body?, action-row?]`. The action row is a
/// right-aligned [`Flex`](crate::widgets::Flex) row of the caller's [`Button`](super::Button)s.
pub struct Dialog {
    base: Base,
    /// When `false`, Esc / scrim clicks are swallowed but don't dismiss — a forced-decision
    /// dialog (the user must pick a button). Default `true`.
    dismissible: bool,

    /// Fired when Esc or a scrim click requests dismissal (only if `dismissible`). The host
    /// points this at its overlay-close path (e.g. emit `CloseOverlay`).
    on_dismiss: Option<Box<dyn Fn()>>,
    /// Whether an action row exists yet (created lazily on the first [`action`](Dialog::action)).
    has_actions: bool,
    /// Where [`body`](Dialog::body) put the body in the panel — remembered when it is placed, so
    /// nothing has to work out which child it is afterwards.
    body_at: Option<usize>,
    /// Set by [`danger`](Dialog::danger).
    danger: bool,
}

impl LayoutExt for Dialog {}
