//! What a dialog is: its id, its buttons, and what it hands back when it closes.

use heca_view::{PropMap, ViewNode};

use crate::app::interaction::InteractionIntent;
use crate::chrome::{ChromeIntentEmitter, LayerId};
use crate::input::WmAction;

/// Opaque, stable id for an open overlay — the same value as its backing
/// [`LayerId`]. Public because [`WmAction`] carries it (an action targets a
/// specific overlay by id); the inner layer id stays crate-private.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OverlayId(pub(crate) LayerId);

impl OverlayId {
    /// **The gesture that closes this overlay without choosing anything** — what Esc, a click
    /// outside and a finished menu run. Posted through `emit`, so it is judged as coming from the
    /// surface that holds the overlay.
    ///
    /// How an overlay is closed, and how it is addressed, is the id's to know: no widget hook
    /// spells the action out.
    pub(crate) fn closer(self, emit: &ChromeIntentEmitter) -> impl Fn() + Clone + 'static {
        let emit = emit.clone();
        move || {
            emit.fire(InteractionIntent::ActivateAction(WmAction::CloseOverlay {
                overlay: Some(self),
            }))
        }
    }

    /// **The gesture that answers this overlay with `action`** — what a dialog's button and a
    /// palette row run. The overlay's completion receives it as [`ModalResult::Action`].
    pub(crate) fn submitter(
        self,
        action: impl Into<String>,
        emit: &ChromeIntentEmitter,
    ) -> impl Fn() + Clone + 'static {
        let emit = emit.clone();
        let action = action.into();
        move || {
            emit.fire(InteractionIntent::ActivateAction(WmAction::SubmitOverlay {
                overlay: Some(self),
                action: action.clone(),
            }))
        }
    }
}

/// One bottom action button of a modal. The author supplies only id/label/danger; the
/// [`OverlayHost`](super::OverlayHost) wires the rest through the same centralized path as every chrome button —
/// a KeyHint target + a tooltip resolved from the action, never a hand-picked shortcut string.
/// `id` doubles as the action name the tooltip resolves its shortcut from (`action_tooltip`).
#[derive(Clone, Debug)]
pub struct ModalAction {
    /// Comes back in [`ModalResult::Action`]; also the action name for the tooltip's shortcut.
    pub id: String,
    pub label: String,
    /// Tint with the danger hue (destructive action).
    pub danger: bool,
    /// If `Some(field)`, this button is **disabled while the named form field is empty** (trimmed)
    /// — used to block submission until a required [`Input`](heca_grid_ui::Input) has content (e.g.
    /// a rename dialog's OK). The host binds the button's disabled state to the field's live value.
    pub disable_when_empty: Option<String>,
}

impl ModalAction {
    /// A plain action button.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            danger: false,
            disable_when_empty: None,
        }
    }
    /// Tint with the danger hue (destructive primary action).
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// Disable this button while the named form field is empty (blocks blank submission).
    pub fn disabled_when_empty(mut self, field: impl Into<String>) -> Self {
        self.disable_when_empty = Some(field.into());
        self
    }
}

/// A modal specification: a title, a declarative body tree, and the bottom action buttons.
pub struct ModalSpec {
    pub title: String,
    /// The dialog body — a full [`ViewNode`] tree (§2.6.2), so a modal can hold a table / form
    /// / list, not just text. [`ModalSpec::message`] wraps a single `Label`.
    pub body: ViewNode,
    pub actions: Vec<ModalAction>,
    /// A destructive question: the dialog says its body in the theme's danger colour
    /// ([`Dialog::danger`](heca_grid_ui::Dialog::danger)). A button's own tint is its [`ModalAction::danger`].
    pub danger: bool,
    /// `false` = forced decision (Esc / scrim swallowed) — mirrors `Dialog::dismissible`.
    pub dismissible: bool,
}

impl ModalSpec {
    /// A simple message modal: `title` over a body that wraps to the panel's width, with no
    /// actions yet (add them with [`action`](ModalSpec::action)). The full warning stays readable.
    pub fn message(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: heca_view::build::Label::new(message).wrap(true).into(),
            actions: Vec::new(),
            danger: false,
            dismissible: true,
        }
    }
    /// Append an action button.
    pub fn action(mut self, action: ModalAction) -> Self {
        self.actions.push(action);
        self
    }
    /// Mark the question destructive — the dialog says its body in the danger colour.
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// Force an explicit choice — Esc / scrim are swallowed without dismissing.
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }
}

/// The resolved outcome of a modal, handed to the caller's completion.
#[derive(Clone, Debug, PartialEq)]
pub enum ModalResult {
    /// A button (or RPC) chose action `id`; `data` carries the modal body's named value fields,
    /// each read at submit time — `Input` → `Text`, `Toggle`/`Checkbox` → `Bool`, `Select` → the
    /// chosen option's value (`Text`). Empty when the body has no named field (e.g. a plain confirm).
    Action { id: String, data: PropMap },
    /// Esc / scrim / `CloseOverlay` — no action chosen.
    Dismissed,
}
