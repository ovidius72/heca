//! **The router** — judges an interaction by its source, the domain and the action's policy: allow it or block it.

use crate::app_state::AppState;
use crate::input::WmAction;
use heca_core::layout::Layout;
use super::types::{InteractionIntent, InteractionSource, RouteDecision};
use super::domain::{Domain, domain_for};
use super::policy::{ActionPolicy, action_policy};


/// Decide whether an interaction is allowed under the current state.
///
/// **One layer, two inputs**: the interaction's [`ActionPolicy`] (what kind of act it is) judged
/// against the current [`Domain`] (what owns the screen). Everything the router used to special-case
/// beside the policy system is a domain now (F003/P086/T371):
///
/// - a modal used to be a blunt "block everything" check right here; it is an overlay that **covers
///   the tiled area**, so `Domain::Overlay` blocks the same things — and a *non-modal* plugin
///   overlay that covers the panes gets the same protection, which the old check could not give it;
/// - a focused container had **no gate at all** — its actions were merely hard to reach by key —
///   and is now `Domain::Container`, the only domain `ActionPolicy::ContainerFocused` permits.
///
/// Overlay *control* (`SubmitOverlay` / `CloseOverlay`) never reaches here — it is intercepted in
/// [`dispatch_intent`] before routing — and an overlay's own Esc / Enter / nav reach the widget
/// through the widget-keymap path, not the WM action system.
pub(crate) fn route_interaction(
    state: &AppState,
    source: InteractionSource,
    intent: InteractionIntent,
) -> RouteDecision {
    route_in_domain(state.layout(), domain_for(state, source), source, intent)
}

/// The core policy function — **pure**, so it stays unit-testable without an `AppState` (which
/// cannot be built without a window). [`route_interaction`] computes the [`Domain`] and hands it
/// the truth rather than re-deriving it here.
///
/// `session` is still needed beside the domain: `SourceDependent` asks *which* pane is the active
/// floating one, which is a fact about the session, not about the domain.
///
/// # What each domain permits
///
/// | policy | `Tiled` | `Floating` | `Container` | `Overlay` |
/// |---|---|---|---|---|
/// | `Global` | ✓ | ✓ | ✓ | ✓ |
/// | `AlwaysAllowed` | ✓ | ✗ | ✓ | ✗ |
/// | `FocusedPaneLocal` | ✓ | ✓ | ✓ | ✗ |
/// | `TiledOnly` | ✓ | ✗ | ✓ | ✗ |
/// | `WorkspaceLevel` | ✓ | ✗ | ✓ | ✗ |
/// | `ContainerFocused` | ✗ | ✗ | ✓ | ✗ |
/// | `SourceDependent` | ✓ | the active floating pane only | ✓ | ✗ |
///
/// `Container` permits what `Tiled` does **plus** `ContainerFocused`: a dock holding the keyboard
/// does not stop `prefix+Enter` from splitting the pane you last worked in, which is the rule the
/// phase settled. `Overlay` permits only `Global`, so `reload_config` keeps working and nothing
/// touches panes the user cannot see.
pub(crate) fn route_in_domain(
    layout: Layout<'_>,
    domain: Domain,
    source: InteractionSource,
    intent: InteractionIntent,
) -> RouteDecision {
    // The three carrier intents below are the mouse's own vocabulary — focus this pane, fold this
    // workspace, start this drag. None of them is meaningful while a floating pane owns the domain
    // or while something covers the panes, and each has always said so; `blocked_domain` is that
    // one condition written once instead of three times.
    let blocked_domain = matches!(domain, Domain::Floating | Domain::Overlay);
    match &intent {
        InteractionIntent::ActivateAction(action) => route_action(layout, domain, source, action),
        // Defensive: `dispatch_intent` expands this into FocusPane + the action before
        // routing, so the router should not normally see it. If it does, route by the
        // inner action's policy (the focus half is always benign).
        InteractionIntent::FocusPaneThenAction { action, .. } => {
            route_action(layout, domain, source, action)
        }
        // Likewise expanded before routing. Defensively it is a `View` intent: the name resolves to
        // its real policy when `dispatch_view_intent` looks it up.
        InteractionIntent::FocusContainerThenAction { .. } => RouteDecision::Allow(intent),
        // Pure UI bookkeeping: it changes nothing anyone could be protected from, and the only
        // time it fires is while a map covers the panes — so refusing it in `Overlay` would refuse
        // it always.
        InteractionIntent::ExposeCursor { .. } => RouteDecision::Allow(intent),
        // FocusPane from mouse content/sidebar: only the active floating pane can receive focus in
        // the floating domain, and nothing behind an overlay can.
        InteractionIntent::FocusPane { .. }
        | InteractionIntent::ToggleWorkspaceCollapsed { .. }
        | InteractionIntent::StartSidebarDrag { .. } => match blocked_domain {
            true => RouteDecision::Block,
            false => RouteDecision::Allow(intent),
        },
        // A View intent is a pass-through here: it carries only an action *name*, so its
        // real focus-domain policy is applied when `dispatch_view_intent` resolves it to a
        // `WmAction` and re-dispatches through `dispatch_action` (which routes it).
        InteractionIntent::View(_) => RouteDecision::Allow(intent),
    }
}

/// Route a WmAction based on the current focus domain and interaction source.
pub(super) fn route_action(
    layout: Layout<'_>,
    domain: Domain,
    source: InteractionSource,
    action: &WmAction,
) -> RouteDecision {
    if policy_allows(layout, domain, source, action_policy(action), Some(action)) {
        RouteDecision::Allow(InteractionIntent::ActivateAction(action.clone()))
    } else {
        RouteDecision::Block
    }
}

/// Does `policy` permit an action in the current focus domain, from this source?
///
/// The **single** place the six policies are interpreted, so a built-in and a plugin action are
/// judged by identical rules. The difference is only where the policy came from: a built-in's is
/// produced by [`action_policy`]'s exhaustive `match` (forgetting a variant is a compile error); a
/// name-keyed action's is **declared** on its `DynActionMeta` (a required field, so forgetting is
/// impossible there too).
///
/// `action` is `Some` only for built-ins — it exists solely for [`ActionPolicy::SourceDependent`],
/// which inspects `FocusPane`'s target. A dynamic action cannot be `FocusPane`, so it takes the
/// conservative branch (blocked while floating), which is the right default for an action the host
/// cannot introspect.
pub(super) fn policy_allows(
    layout: Layout<'_>,
    domain: Domain,
    source: InteractionSource,
    policy: ActionPolicy,
    action: Option<&WmAction>,
) -> bool {
    // Nothing but a true global act reaches past something covering the panes — acting on what the
    // user cannot see is the whole reason this domain exists (F003/P086/T371). It replaces the
    // blunt "a modal blocks everything" check the router did before policy was consulted at all,
    // and unlike that one it also covers a non-modal overlay that declared it obscures the panes.
    if domain == Domain::Overlay {
        return policy == ActionPolicy::Global;
    }
    let floating = domain == Domain::Floating;

    match policy {
        // True global app actions (ReloadConfig) — allowed in every focus domain, including
        // Floating. They have no tiled/floating layout impact, so blocking them when floating only
        // breaks hot-reload.
        ActionPolicy::Global => true,
        // AlwaysAllowed (CommandPalette, SpawnCommand) is a misnomer: it is blocked when
        // floating from the current sources. Future chrome sources (MouseTopMenu, MouseStatusBar)
        // may allow these even while floating.
        ActionPolicy::AlwaysAllowed => !floating,
        ActionPolicy::FocusedPaneLocal => true,
        ActionPolicy::TiledOnly => !floating,
        ActionPolicy::WorkspaceLevel => !floating,
        // A component's own selection-dependent verb: only while a dock is being driven. This is
        // what makes the palette and RPC paths safe without the component declaring anything about
        // where it can be reached from.
        ActionPolicy::ContainerFocused => domain == Domain::Container,
        ActionPolicy::SourceDependent => {
            // FocusPane: allowed if it targets the active floating pane, otherwise blocked.
            if let Some(WmAction::FocusPane { pane_id }) = action {
                if !floating {
                    return true;
                }
                let active_floating = layout
                    .active_workspace()
                    .and_then(|ws| ws.content().floating_panes.iter().find(|f| f.is_active))
                    .map(|f| f.pane.id);
                return active_floating == Some(*pane_id);
            }
            // Other source-dependent actions: defer to source when floating.
            if floating {
                match source {
                    InteractionSource::Keyboard => false,
                    InteractionSource::MouseContent => false,
                    InteractionSource::MouseLeftSidebar => false,
                    // A component gets no more reach than the user driving it: if the same request
                    // would be blocked from a key while a pane is floating, asking for it from
                    // inside `perform` must be blocked too.
                    InteractionSource::Provider => false,
                    // Nor does a script (F003/P086/T372).
                    InteractionSource::Rpc => false,
                    // Nor a surface acting on itself: a layer is a place a declaration was made,
                    // not a licence to reach past the floating domain (F003/P082/T416).
                    InteractionSource::Surface(_) => false,
                }
            } else {
                true
            }
        }
    }
}
