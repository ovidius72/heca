//! `OverlayHost` — the host-owned overlay stack (plan §2.7.1 / §2.7.2), built **on** the
//! [`LayerRegistry`](super::LayerRegistry) rather than as a parallel stack.
//!
//! A caller (a handler, or later a plugin/RPC) submits a [`ModalSpec`] and a completion; the
//! host [`realize`](super::realize)s the spec's `body` [`ViewNode`], injects a real
//! [`Button`](heca_grid_ui::Button) per [`ModalAction`] — each carrying the overlay's id via
//! [`WmAction::SubmitOverlay`] — wraps them in a [`Dialog`](heca_grid_ui::Dialog), and pushes
//! it as a `Modal`-band layer. Because the buttons are real components they are hint targets
//! and focus-traversable for free. Confirmation/dismissal come back as overlay-control actions
//! (`SubmitOverlay` / `CloseOverlay`) resolved by [`resolve`] — the single point that pops the
//! layer and runs the caller's completion. RPC drives the exact same actions by id.
//!
//! Seam module: `open_modal` is consumed by the confirm-dialog migration (and later plugins);
//! carries `#![allow(dead_code)]` like the sibling chrome seam modules until then.
#![allow(dead_code)]

use std::collections::HashMap;
use std::rc::Rc;

use heca_grid_ui::{Button, ButtonVariant, Component, Dialog, HintExt};

use super::view::{PropMap, ViewNode, WidgetKind};
use super::{ChromeIntentEmitter, LayerBand, LayerId, LayerKind};
use crate::actions::ActionRegistry;
use crate::app::events::AppEvent;
use crate::app::interaction::{InteractionIntent, InteractionSource};
use crate::app_state::AppState;
use crate::input::WmAction;

/// Opaque, stable id for an open overlay — the same value as its backing
/// [`LayerId`](super::LayerId). Public because [`WmAction`] carries it (an action targets a
/// specific overlay by id); the inner layer id stays crate-private.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OverlayId(pub(crate) LayerId);

/// One bottom action button of a modal. The author supplies only id/label/danger/shortcut;
/// the [`OverlayHost`] injects the overlay id when it builds the button.
#[derive(Clone, Debug)]
pub struct ModalAction {
    /// Comes back in [`ModalResult::Action`] — a button id or a plugin action id.
    pub id: String,
    pub label: String,
    /// Tint with the danger hue (destructive action).
    pub danger: bool,
    /// Optional letter that fires this action from the keyboard (e.g. `y` / `n`), resolved by
    /// the overlay input path against the top modal's actions.
    pub shortcut: Option<char>,
}

impl ModalAction {
    /// A plain action button.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            danger: false,
            shortcut: None,
        }
    }
    /// Tint with the danger hue (destructive primary action).
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    /// A letter shortcut that fires this action from the keyboard.
    pub fn shortcut(mut self, c: char) -> Self {
        self.shortcut = Some(c);
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
    /// Tint the panel/primary action as destructive.
    pub danger: bool,
    /// `false` = forced decision (Esc / scrim swallowed) — mirrors `Dialog::dismissible`.
    pub dismissible: bool,
}

impl ModalSpec {
    /// A simple message modal: `title` over a one-line body `message`, no actions yet (add
    /// them with [`action`](ModalSpec::action)).
    pub fn message(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: ViewNode::new(WidgetKind::Label).text(message),
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
    /// Mark destructive (tints the primary action).
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
    /// A button (or RPC) chose action `id`; `data` carries anything the body collected (empty
    /// for now — form/table marshalling is a later step).
    Action { id: String, data: PropMap },
    /// Esc / scrim / `CloseOverlay` — no action chosen.
    Dismissed,
}

/// A caller's completion, run once when the overlay resolves. It receives the live
/// [`AppState`] + [`ActionRegistry`] (so it can dispatch a follow-up action) and the result.
type OverlayCompletion = Box<dyn FnOnce(&mut AppState, &ActionRegistry, ModalResult)>;

/// The host-owned overlay stack: pending completions keyed by overlay id. The overlay's
/// *visual* stack lives in the [`LayerRegistry`](super::LayerRegistry); this holds only the
/// result callbacks. Kept on [`AppState`].
#[derive(Default)]
pub struct OverlayHost {
    completions: HashMap<OverlayId, OverlayCompletion>,
    /// Action metadata per open overlay (id + shortcut), so the overlay input path can resolve
    /// a letter key to an action and Esc to dismissal. Mirrors the layer stack's lifetime.
    actions: HashMap<OverlayId, Vec<ModalAction>>,
}

impl OverlayHost {
    /// The action metadata for an open overlay (for the keyboard input path).
    pub(crate) fn actions_for(&self, overlay: OverlayId) -> Option<&[ModalAction]> {
        self.actions.get(&overlay).map(|v| v.as_slice())
    }
}

/// The front-most open **modal** overlay — the one capturing input — if any. The input path
/// routes keyboard/pointer to it and swallows everything else while it's up.
pub(crate) fn top_modal(state: &AppState) -> Option<OverlayId> {
    state.layers.top_modal_id().map(OverlayId)
}

/// The action id whose letter shortcut is `c` in overlay `overlay`, if any — the keyboard
/// letter path (e.g. `y`/`n`) resolves a key to the action id it fires via `SubmitOverlay`.
pub(crate) fn shortcut_action(state: &AppState, overlay: OverlayId, c: char) -> Option<String> {
    state
        .overlays
        .actions_for(overlay)?
        .iter()
        .find(|a| a.shortcut.is_some_and(|s| s.eq_ignore_ascii_case(&c)))
        .map(|a| a.id.clone())
}

/// Open a modal: realize its body + inject id-carrying action buttons, push it as a
/// `Modal`-band layer, and register `completion` to run when it resolves. Returns the
/// [`OverlayId`] (RPC keeps it to drive `SubmitOverlay`/`CloseOverlay`).
pub(crate) fn open_modal(
    state: &mut AppState,
    spec: ModalSpec,
    completion: impl FnOnce(&mut AppState, &ActionRegistry, ModalResult) + 'static,
) -> OverlayId {
    // Reserve the id first so the action buttons can carry it (they're built before the layer
    // is inserted).
    let id = OverlayId(state.layers.reserve_id());

    // The chrome intent sink — same shape as the retained chrome tree's emitter (mod.rs): a
    // button's `on_click` posts an `AppEvent::ChromeIntent`, dispatched by the event loop.
    let event_proxy = state.event_proxy.clone();
    let emit: ChromeIntentEmitter = Rc::new(move |intent| {
        let _ = event_proxy.send_event(AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent,
        });
    });

    let actions = spec.actions.clone();
    let dismissible = spec.dismissible;
    let root = build_modal_root(&spec, id, &emit, &mut state.hint_targets);
    state.layers.insert(
        id.0,
        LayerBand::Modal,
        LayerKind::OnDemand,
        true,
        root,
    );
    state.overlays.completions.insert(id, Box::new(completion));
    state.overlays.actions.insert(id, actions);
    let _ = dismissible;
    state.needs_redraw = true;
    id
}

/// Build the realized `Dialog` tree for a modal (pure: no `AppState`, so it's unit-testable).
/// Each action becomes a real `Button` carrying `SubmitOverlay { overlay: id, action: id }`
/// as both its click intent and its hint target — one intent, both input paths.
fn build_modal_root(
    spec: &ModalSpec,
    id: OverlayId,
    emit: &ChromeIntentEmitter,
    hints: &mut super::HintTargetRegistry,
) -> Box<dyn Component> {
    let body = super::realize(&spec.body, emit, hints);
    let mut dialog = Dialog::new(spec.title.clone()).body_boxed(body);
    for action in &spec.actions {
        let variant = if action.danger {
            ButtonVariant::Destructive
        } else {
            ButtonVariant::Secondary
        };
        let carrier = InteractionIntent::ActivateAction(WmAction::SubmitOverlay {
            overlay: id,
            action: action.id.clone(),
        });
        let hid = hints.register(carrier.clone());
        let emit = emit.clone();
        let button = Button::new(action.label.clone())
            .variant(variant)
            .hint_target(hid)
            .on_click(move || emit(carrier.clone()));
        dialog = dialog.action(button);
    }
    // Esc / scrim dismissal flows through the same emitter as the buttons: a `CloseOverlay`
    // for this overlay, resolved to `ModalResult::Dismissed` in `dispatch_intent`.
    let emit_dismiss = emit.clone();
    let close = InteractionIntent::ActivateAction(WmAction::CloseOverlay { overlay: id });
    Box::new(
        dialog
            .dismissible(spec.dismissible)
            .on_dismiss(move || emit_dismiss(close.clone()))
            .open(true),
    )
}

/// Resolve an open overlay: pop its layer + action metadata and run its completion with
/// `result`. The single confirm/dismiss point — a button press, a KeyHint pick, and RPC all
/// funnel here via `SubmitOverlay`/`CloseOverlay` (dispatched in `dispatch_intent`). No-op if
/// the overlay is already gone (double-resolve safe).
pub(crate) fn resolve(
    state: &mut AppState,
    registry: &ActionRegistry,
    overlay: OverlayId,
    result: ModalResult,
) {
    let completion = state.overlays.completions.remove(&overlay);
    state.overlays.actions.remove(&overlay);
    state.layers.remove(overlay.0);
    state.needs_redraw = true;
    if let Some(comp) = completion {
        comp(state, registry, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::view::PropValue;

    fn noop_emit() -> ChromeIntentEmitter {
        Rc::new(|_| {})
    }

    #[test]
    fn message_spec_wraps_body_in_a_label() {
        let spec = ModalSpec::message("Delete pane?", "This cannot be undone.");
        assert_eq!(spec.body.kind, WidgetKind::Label);
        assert_eq!(
            spec.body.props.get("text").and_then(PropValue::as_text),
            Some("This cannot be undone."),
        );
        assert!(spec.dismissible && !spec.danger && spec.actions.is_empty());
    }

    #[test]
    fn build_registers_one_submit_intent_per_action() {
        let spec = ModalSpec::message("Delete pane?", "Gone forever.")
            .action(ModalAction::new("cancel", "Cancel").shortcut('n'))
            .action(ModalAction::new("confirm", "Delete").danger(true).shortcut('y'))
            .dismissible(false);
        let id = OverlayId(super::super::LayerRegistry::default().reserve_id());
        let mut hints = super::super::HintTargetRegistry::default();
        let before = hints.checkpoint();
        let root = build_modal_root(&spec, id, &noop_emit(), &mut hints);

        // Two actions → two hint targets, each a SubmitOverlay for this overlay.
        assert_eq!(hints.checkpoint() - before, 2);
        for (offset, action_id) in [(0, "cancel"), (1, "confirm")] {
            let intent = hints
                .get(heca_grid_ui::HintTargetId::new(before + offset))
                .unwrap();
            assert!(
                matches!(
                    intent,
                    InteractionIntent::ActivateAction(WmAction::SubmitOverlay { overlay, action })
                        if *overlay == id && action == action_id
                ),
                "target {offset} should submit '{action_id}' to this overlay, got {intent:?}",
            );
        }

        // Structure: the Dialog root has one panel child holding [title, body, action-row(2)].
        let panel = &root.base().children[0];
        assert_eq!(panel.base().children.len(), 3, "title + body + action row");
        assert_eq!(panel.base().children[2].base().children.len(), 2, "two buttons");
    }
}
