//! **Dispatch** — the chokepoint every user-initiated action goes through: overlay control, the focus-then-act composite, and `dispatch_action`.

use super::route::route_interaction;
use super::types::{InteractionIntent, InteractionSource, RouteDecision};
use super::view_intent::{IntentOutcome, dispatch_view_intent};
use crate::actions::ActionRegistry;
use crate::app_state::AppState;
use crate::chrome::Intent as ViewIntent;
use crate::input::WmAction;
use crate::keymap::ActionRef;

/// The single public entry point for user-initiated WM actions.
///
/// Routes the action through the interaction policy layer. If allowed,
/// executes via the registry. If blocked, silently discards.
///
/// Handler-to-handler calls should use `registry.execute()` directly —
/// they bypass the router because they're inside an already-allowed
/// interaction.
/// The overlay an overlay-control action acts on, in this order:
///
/// 1. the one it **names**;
/// 2. the one it **came from** — a dialog's button is the dialog acting on itself, and its
///    [`source`](InteractionSource::Surface) says which one. This is what lets a button submit its
///    own overlay without carrying a runtime id the wire cannot hold (`overlay` is `#[serde(skip)]`),
///    and it is right where "the front-most" is not: a dialog's button pressed while another modal
///    is in front of it;
/// 3. bare, as from a bound key or an RPC line, which cannot name a runtime id — the front-most
///    visible modal layer.
///
/// `None` when nothing is open.
pub(super) fn target_overlay(
    layers: &crate::chrome::LayerRegistry,
    window: &heca_grid_ui::widgets::Flex,
    source: InteractionSource,
    named: Option<crate::chrome::OverlayId>,
) -> Option<crate::chrome::OverlayId> {
    named
        .or_else(|| match source {
            InteractionSource::Surface(key) => layers.id_of_surface(key).map(crate::chrome::OverlayId),
            _ => None,
        })
        .or_else(|| layers.top_modal_id(window).map(crate::chrome::OverlayId))
}

/// Resolve an overlay-control action ([`WmAction::is_overlay_control`]) — the only place they run.
pub(super) fn resolve_overlay_control(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    action: &WmAction,
) {
    let (named, result) = match action {
        WmAction::SubmitOverlay { overlay, action } => (*overlay, Some(action.clone())),
        WmAction::CloseOverlay { overlay } => (*overlay, None),
        _ => return,
    };
    let Some(overlay) = target_overlay(&state.layers, &state.window_root, source, named) else {
        return;
    };
    let result = match result {
        // Marshal the modal body's named value fields (input/toggle/checkbox) into the result
        // `data` before the overlay is popped.
        Some(id) => crate::chrome::ModalResult::Action {
            id,
            data: crate::chrome::collect_overlay_form(state, overlay),
        },
        // A named host layer (the exposé) has no completion to resolve, so it is simply hidden; an
        // overlay with one is resolved as a dismissal, which is what pops it and runs its completion.
        None => crate::chrome::ModalResult::Dismissed,
    };
    crate::chrome::resolve_overlay(state, registry, overlay, result);
}

pub(crate) fn dispatch_intent(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    intent: InteractionIntent,
) {
    // Overlay control (§2.7.2): `SubmitOverlay`/`CloseOverlay` are resolved here — not via the
    // `ActionRegistry` — because resolving an overlay runs its completion, which needs the
    // registry to dispatch a follow-up action (a handler gets no registry). Intercepted before
    // routing, like the `FocusPaneThenAction` composite below. Always allowed (overlay control
    // has no focus-domain policy; the follow-up action it dispatches is routed on its own).
    if let InteractionIntent::ActivateAction(action) = &intent
        && action.is_overlay_control()
    {
        resolve_overlay_control(state, registry, source, action);
        return;
    }

    // The container half of the same composite. Its outcome is dropped here — a click and a menu
    // pick have nowhere to report one — while RPC calls the same function for the answer.
    if let InteractionIntent::FocusContainerThenAction { container, action } = &intent {
        let (container, action) = (container.clone(), action.clone());
        focus_container_then_action(state, registry, source, &container, &action);
        return;
    }

    // Composite: focus the pane, then run the action — each half policy-routed on its
    // own (mirrors what an active-targeted pane button does across two events on click).
    if let InteractionIntent::FocusPaneThenAction { pane_id, action } = intent {
        dispatch_intent(
            state,
            registry,
            source,
            InteractionIntent::FocusPane { pane_id },
        );
        dispatch_intent(
            state,
            registry,
            source,
            InteractionIntent::ActivateAction(*action),
        );
        return;
    }
    // Kept for the debug line below: routing consumes the intent, and a refusal that cannot name
    // what was refused is not actionable.
    #[cfg(debug_assertions)]
    let refused = intent.clone();
    let decision = route_interaction(state, source, intent);

    match decision {
        RouteDecision::Allow(InteractionIntent::ActivateAction(act)) => {
            // Central destructive-action gate: close-pane / delete-column / delete-workspace go
            // through the confirm chokepoint FIRST, so the confirm guard lives on the action and
            // every surface (keyboard, pane-header close button, context menu / dropdown, RPC)
            // confirms identically — never per call site. Returns true when it handled (confirmed
            // or ran) the action; the confirm dialog runs the raw action directly, never re-entering
            // this gate.
            if !crate::handlers::maybe_confirm_destructive(state, &act) {
                registry.execute(&act, state);
            }
        }
        // Both composites are expanded before routing, so neither reaches here in practice;
        // handled defensively as focus-then-act rather than silently dropped.
        RouteDecision::Allow(InteractionIntent::FocusContainerThenAction { container, action }) => {
            focus_container_then_action(state, registry, source, &container, &action);
        }
        RouteDecision::Allow(InteractionIntent::FocusPaneThenAction { pane_id, action }) => {
            registry.execute(&WmAction::FocusPane { pane_id }, state);
            registry.execute(&action, state);
        }
        RouteDecision::Allow(InteractionIntent::FocusPane { pane_id }) => {
            registry.execute(&WmAction::FocusPane { pane_id }, state);
        }
        RouteDecision::Allow(InteractionIntent::ExposeCursor { pane_id }) => {
            crate::chrome::record_expose_cursor(state, pane_id);
        }
        RouteDecision::Allow(InteractionIntent::ToggleWorkspaceCollapsed { ws_idx }) => {
            crate::handlers::apply_ws_collapse(state, ws_idx, None);
        }
        RouteDecision::Allow(InteractionIntent::StartSidebarDrag { .. }) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] interaction: StartSidebarDrag intent allowed but not dispatched (drag initiated in mouse layer)"
            );
        }
        RouteDecision::Allow(InteractionIntent::View(vi)) => {
            dispatch_view_intent(state, registry, source, &vi);
        }
        RouteDecision::Block => {
            // **Say WHAT was refused, not only who asked.** The source alone cannot be acted on: a
            // single click can send more than one intent, so identical lines may be different
            // refusals — and one of them being correct says nothing about the others.
            #[cfg(debug_assertions)]
            eprintln!("[heca] interaction: blocked {refused:?} from {source:?}");
        }
    }
}

/// Focus `container`, then dispatch `action` at it — the one implementation behind
/// [`InteractionIntent::FocusContainerThenAction`], shared by the palette and RPC.
///
/// **The focus half is skipped when that container already holds the keyboard**, and this is not an
/// optimization: `focus_dock` aimed at the focused dock is deliberately a *toggle* (it is the way
/// back out), so focusing unconditionally would release the keyboard and leave the
/// action to be refused by the very policy this exists to satisfy.
///
/// Returns what became of the **action** — the focus half is a means, not the answer a caller asked
/// for. A container that is not mounted is [`IntentOutcome::NotRunnable`]: nothing is focused and
/// nothing runs, rather than the action falling through to whatever else declares that name.
pub(crate) fn focus_container_then_action(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    container: &str,
    action: &ViewIntent,
) -> IntentOutcome {
    if state.chrome_host.provider(container).is_none() {
        #[cfg(debug_assertions)]
        eprintln!("[heca] interaction: no container mounted as '{container}'");
        return IntentOutcome::NotRunnable;
    }
    if crate::app::tree_focus::focused_dock(state).as_deref() != Some(container) {
        dispatch_action(
            state,
            registry,
            source,
            &WmAction::FocusDock {
                dock: Some(container.to_string()),
            },
        );
    }
    dispatch_view_intent(state, registry, source, action)
}

pub(crate) fn dispatch_action(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    action: &WmAction,
) {
    dispatch_intent(
        state,
        registry,
        source,
        InteractionIntent::ActivateAction(action.clone()),
    );
}

/// Dispatch whatever a key was bound to — the press-time half of [`ActionRef`].
///
/// A `Builtin` was already resolved at config load and dispatches exactly as it always has. A
/// `Dynamic` names an action that may only have been registered *after* config was read (a provider,
/// a plugin); it is resolved **now**, through the same one door as every other named intent, so it
/// is policy-routed identically. Still unknown at press ⇒ a debug warning inside
/// `dispatch_view_intent`, never a crash.
pub(crate) fn dispatch_action_ref(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    action: &ActionRef,
) {
    match action {
        ActionRef::Builtin(a) => dispatch_action(state, registry, source, a),
        ActionRef::Dynamic(intent) => dispatch_intent(
            state,
            registry,
            source,
            InteractionIntent::View(intent.clone()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::interaction::SurfaceKey;
    use crate::chrome::{LayerKind, LayerRegistry, OverlayId, new_window_root};
    use heca_grid_ui::WidgetModel;
    use heca_grid_ui::widgets::Flex;

    fn dialog() -> WidgetModel {
        WidgetModel::new(Flex::column())
    }

    /// **A dialog's button submits its own overlay.** The id is a runtime counter the wire cannot
    /// carry, so the act arrives stamped with the surface it came from and resolves it from that —
    /// even when another surface was opened after it and would be "the front-most".
    #[test]
    fn a_submit_from_a_surface_resolves_that_surface_and_a_bare_one_the_front_most() {
        let mut layers = LayerRegistry::default();
        let mut window = new_window_root();
        let (first, second) = (layers.reserve_id(), layers.reserve_id());
        layers.insert(first, None, LayerKind::OnDemand, dialog(), &mut window);
        layers.insert(second, None, LayerKind::OnDemand, dialog(), &mut window);

        let from_first = InteractionSource::Surface(layers.surface_key(first));
        assert_eq!(
            target_overlay(&layers, &window, from_first, None),
            Some(OverlayId(first)),
            "its own overlay, not the one opened after it"
        );
        let from_second = InteractionSource::Surface(layers.surface_key(second));
        assert_eq!(
            target_overlay(&layers, &window, from_second, None),
            Some(OverlayId(second))
        );
        assert_eq!(
            target_overlay(&layers, &window, from_first, Some(OverlayId(second))),
            Some(OverlayId(second)),
            "one that names an overlay still wins"
        );
        assert_eq!(
            target_overlay(&layers, &window, InteractionSource::Surface(SurfaceKey::of("gone")), None),
            layers.top_modal_id(&window).map(OverlayId),
            "a surface that is no longer a layer falls back to the front-most"
        );
        assert_eq!(
            target_overlay(&layers, &window, InteractionSource::Keyboard, None),
            layers.top_modal_id(&window).map(OverlayId),
            "a bound key or an RPC line has no surface and acts on the front-most"
        );
    }
}
