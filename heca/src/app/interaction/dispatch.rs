//! **Dispatch** — the chokepoint every user-initiated action goes through: overlay control, the focus-then-act composite, and `dispatch_action`.

use crate::actions::ActionRegistry;
use crate::app_state::AppState;
use crate::chrome::Intent as ViewIntent;
use crate::input::WmAction;
use crate::keymap::ActionRef;
use super::types::{InteractionIntent, InteractionSource, RouteDecision};
use super::route::route_interaction;
use super::view_intent::{IntentOutcome, dispatch_view_intent};


/// The single public entry point for user-initiated WM actions.
///
/// Routes the action through the interaction policy layer. If allowed,
/// executes via the registry. If blocked, silently discards.
///
/// Handler-to-handler calls should use `registry.execute()` directly —
/// they bypass the router because they're inside an already-allowed
/// interaction.
/// The overlay an overlay-control action acts on: the one it names, or — bare, as from a bound key
/// or an RPC line, which cannot name a runtime id — the front-most visible modal layer. `None` when
/// nothing is open.
pub(super) fn target_overlay(
    state: &AppState,
    named: Option<crate::chrome::OverlayId>,
) -> Option<crate::chrome::OverlayId> {
    named.or_else(|| {
        state
            .layers
            .top_modal_id(&state.window_root)
            .map(crate::chrome::OverlayId)
    })
}

/// Resolve an overlay-control action ([`WmAction::is_overlay_control`]) — the only place they run.
pub(super) fn resolve_overlay_control(state: &mut AppState, registry: &ActionRegistry, action: &WmAction) {
    let (named, result) = match action {
        WmAction::SubmitOverlay { overlay, action } => (*overlay, Some(action.clone())),
        WmAction::CloseOverlay { overlay } => (*overlay, None),
        _ => return,
    };
    let Some(overlay) = target_overlay(state, named) else {
        return;
    };
    let result = match result {
        // Marshal the modal body's named value fields (input/toggle/checkbox) into the result
        // `data` before the overlay is popped (plugin-task-ui-4).
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
        resolve_overlay_control(state, registry, action);
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
            // refusals — and one of them being correct says nothing about the others
            // (Antonio, driving, 2026-09-02).
            #[cfg(debug_assertions)]
            eprintln!("[heca] interaction: blocked {refused:?} from {source:?}");
        }
    }
}

/// Focus `container`, then dispatch `action` at it — the one implementation behind
/// [`InteractionIntent::FocusContainerThenAction`], shared by the palette and RPC
/// (F003/P085/T358).
///
/// **The focus half is skipped when that container already holds the keyboard**, and this is not an
/// optimization: `focus_dock` aimed at the focused dock is deliberately a *toggle* (it is the way
/// back out, F003/P085/T352), so focusing unconditionally would release the keyboard and leave the
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
    if state.chrome_state.focused_container().as_deref() != Some(container) {
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
