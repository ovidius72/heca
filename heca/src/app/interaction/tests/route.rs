//! The router: which interactions are allowed in which domain, from which source.

use super::*;


/// In Tiled domain, TiledOnly actions are allowed from any source.
#[test]
fn tiled_domain_allows_tiled_actions() {
    let session = test_session();
    let actions = [
        WmAction::FocusLeft,
        WmAction::FocusRight,
        WmAction::SplitHorizontal,
        WmAction::ZoomColumn,
        WmAction::SidebarLeft,
    ];
    for action in &actions {
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Allow(_)),
            "Tiled domain should allow {:?} but got {:?}",
            action,
            decision,
        );
    }
}


/// A View intent is a pass-through at the router: it carries only an action name, so
/// the router always allows it and real policy is applied when it resolves to a
/// `WmAction` and re-dispatches. Also exercises constructing the `View` variant.
#[test]
fn view_intent_passes_through_router() {
    let session = test_session();
    let decision = route_in_domain(
        session.l(),
        session_domain(session.l()),
        InteractionSource::Keyboard,
        InteractionIntent::View(ViewIntent::new("focus_left")),
    );
    assert!(
        matches!(decision, RouteDecision::Allow(InteractionIntent::View(_))),
        "router should pass a View intent through, got {:?}",
        decision,
    );
}


/// In Tiled domain, FocusedPaneLocal actions are allowed.
#[test]
fn tiled_domain_allows_focused_pane_local() {
    let session = test_session();
    let actions = [
        WmAction::Float,
        WmAction::ClosePane,
        WmAction::RenamePane,
        // Selection (host capability, Task 02).
        WmAction::EnterSelectionMode,
        WmAction::SelectionLeft,
        WmAction::SelectionRight,
        WmAction::SelectionUp,
        WmAction::SelectionDown,
        WmAction::ClearSelection,
        WmAction::CopySelection,
        WmAction::PasteClipboard,
        WmAction::OpenLinkAtCaret,
    ];
    for action in &actions {
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Allow(_)),
            "Tiled domain should allow {:?} but got {:?}",
            action,
            decision,
        );
    }
}


/// **The user's case**: a plugin opens a non-modal overlay over the scrolling area,
/// and `prefix+Enter` must not add a pane behind it. No plugin declares anything about
/// `split_horizontal` — it declares that its overlay covers the panes, and every `TiledOnly`
/// act falls away with no further rule (F003/P086/T371).
#[test]
fn an_overlay_covering_the_panes_blocks_acting_on_them() {
    let session = test_session();
    for action in [
        WmAction::SplitHorizontal,
        WmAction::ZoomColumn,
        WmAction::ClosePane,
        WmAction::FocusLeft,
        WmAction::CreateWorkspace,
    ] {
        let decision = route_in_domain(
            session.l(),
            Domain::Overlay,
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "{action:?} must not reach a pane the user cannot see, got {decision:?}",
        );
    }
    // …and a true global act still gets through, or a covered screen would also mean no
    // `reload_config`.
    assert!(matches!(
        route_in_domain(
            session.l(),
            Domain::Overlay,
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(WmAction::ReloadConfig),
        ),
        RouteDecision::Allow(_),
    ));
}


/// **The exposé's defect, as a rule.** The map declares `lock: false` — you can see
/// the panes through it, and that is geometrically true — so the old gate said the base context
/// was still live and `prefix+j` drove the session behind it. What makes
/// a context active is that it took the keyboard, not what it painted over.
#[test]
fn a_surface_that_took_the_keyboard_makes_the_base_context_dormant_even_if_it_covers_nothing() {
    assert!(base_context_is_dormant(
        Some(surface("heca.expose")),
        false, // covers nothing — a map of the panes is not a lid over them
        InteractionSource::Keyboard,
    ));
}


/// The exception that keeps T327's deletes working, and the one a plugin's layer relies on:
/// the surface holding the keyboard is not "the app being driven behind it". `x` on a card is
/// the map's own declaration and is judged by the action's policy, like anyone else's.
#[test]
fn the_active_surface_acting_on_itself_is_not_refused() {
    assert!(!base_context_is_dormant(
        Some(surface("heca.expose")),
        false,
        InteractionSource::Surface(surface("heca.expose")),
    ));
}


/// …and it is the *active* surface, not any surface. A layer that no longer holds the keyboard
/// — one beneath the dialog that opened over it — gets no reach past it.
#[test]
fn a_surface_underneath_the_active_one_gets_no_reach() {
    assert!(base_context_is_dormant(
        Some(surface("heca.confirm")),
        false,
        InteractionSource::Surface(surface("heca.expose")),
    ));
}


/// With nothing holding the keyboard, coverage still means what it says: something opaque over
/// the tiled area means the panes are not what the user is looking at.
#[test]
fn coverage_still_decides_when_no_surface_holds_the_keyboard() {
    assert!(base_context_is_dormant(
        None,
        true,
        InteractionSource::Keyboard
    ));
    assert!(!base_context_is_dormant(
        None,
        false,
        InteractionSource::Keyboard
    ));
}


/// A component's cursor verb is reachable **only while its dock is being driven** — which is
/// what closes the palette/RPC hole those actions had (F003/P086/T371).
#[test]
fn a_container_focused_action_needs_a_focused_container() {
    let session = test_session();
    assert!(policy_allows(
        session.l(),
        Domain::Container,
        InteractionSource::Keyboard,
        ActionPolicy::ContainerFocused,
        None,
    ));
    for domain in [Domain::Tiled, Domain::Floating, Domain::Overlay] {
        assert!(
            !policy_allows(
                session.l(),
                domain,
                InteractionSource::Keyboard,
                ActionPolicy::ContainerFocused,
                None,
            ),
            "{domain:?} is not a dock being driven",
        );
    }
}


/// **A script is judged like a key, not like a click** (F003/P085/T358).
///
/// A click lands *on* something and is judged by what it landed on; a script lands on nothing,
/// so where the keyboard is *is* the context. Without this, `heca action workspaces.delete_row`
/// was refused even with the workspaces dock focused — the policy T371 introduced had no door at
/// all from RPC, which is what `FocusContainerThenAction` exists to open.
#[test]
fn a_script_is_judged_where_the_keyboard_is() {
    let session = test_session();
    for source in [
        InteractionSource::Rpc,
        InteractionSource::Keyboard,
        InteractionSource::Provider,
    ] {
        assert!(
            policy_allows(
                session.l(),
                Domain::Container,
                source,
                ActionPolicy::ContainerFocused,
                None,
            ),
            "{source:?} drives the focused dock",
        );
    }
    // And it buys a script nothing else: with no dock focused it is refused exactly as a
    // keypress is, which is why focusing first is the composite's job and not a special case.
    assert!(!policy_allows(
        session.l(),
        Domain::Tiled,
        InteractionSource::Rpc,
        ActionPolicy::ContainerFocused,
        None,
    ));
}


/// The composite is expanded before routing, so the router only ever sees it defensively — and
/// passes it through, because the action half carries a *name* whose real policy is read when
/// `dispatch_view_intent` resolves it.
#[test]
fn the_focus_container_composite_passes_through_the_router() {
    let session = test_session();
    for domain in [Domain::Tiled, Domain::Container, Domain::Overlay] {
        assert!(
            matches!(
                route_in_domain(
                    session.l(),
                    domain,
                    InteractionSource::Keyboard,
                    InteractionIntent::FocusContainerThenAction {
                        container: "workspaces".to_string(),
                        action: ViewIntent::new("workspaces.delete_row"),
                    },
                ),
                RouteDecision::Allow(InteractionIntent::FocusContainerThenAction { .. }),
            ),
            "{domain:?}: the name is judged on resolution, not here",
        );
    }
}


/// A dock holding the keyboard does not stop the app's own tiled actions — the phase's rule
/// that `prefix+…` keeps working while a container is focused, expressed as a domain.
#[test]
fn a_focused_container_still_permits_the_tiled_actions() {
    let session = test_session();
    for policy in [
        ActionPolicy::TiledOnly,
        ActionPolicy::WorkspaceLevel,
        ActionPolicy::AlwaysAllowed,
        ActionPolicy::FocusedPaneLocal,
        ActionPolicy::Global,
    ] {
        assert!(
            policy_allows(
                session.l(),
                Domain::Container,
                InteractionSource::Keyboard,
                policy,
                None,
            ),
            "{policy:?} should still run while a dock has the keyboard",
        );
    }
}


// ── plugin-04 / T3: a name-keyed action is judged by IDENTICAL rules ──

/// A name-keyed (plugin) action's DECLARED policy runs through the very same `policy_allows`
/// the built-in path reaches — so declaring `TiledOnly` blocks it when a floating pane owns the
/// domain, exactly as if it were a `WmAction` classified `TiledOnly` in the exhaustive match.
#[test]
fn a_declared_tiled_only_action_is_blocked_when_floating() {
    let mut session = test_session();
    // Tiled: allowed.
    assert!(policy_allows(
        session.l(),
        Domain::Tiled,
        InteractionSource::Keyboard,
        ActionPolicy::TiledOnly,
        None
    ));
    // Floating: blocked.
    session.ws().focus_domain = FocusDomain::Floating;
    assert!(!policy_allows(
        session.l(),
        session_domain(session.l()),
        InteractionSource::Keyboard,
        ActionPolicy::TiledOnly,
        None
    ));
}


/// `Global` is the only policy that is truly always allowed (the `AlwaysAllowed` name is a trap
/// — the router blocks THAT one when floating). A plugin action declaring `Global` keeps working
/// while a floating pane owns the domain.
#[test]
fn a_declared_global_action_survives_the_floating_domain() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;
    assert!(policy_allows(
        session.l(),
        session_domain(session.l()),
        InteractionSource::Keyboard,
        ActionPolicy::Global,
        None
    ));
    assert!(
        !policy_allows(
            session.l(),
            session_domain(session.l()),
            InteractionSource::Keyboard,
            ActionPolicy::AlwaysAllowed,
            None
        ),
        "AlwaysAllowed is a misnomer: the router blocks it when floating"
    );
}


/// `SourceDependent` with no introspectable action (the name-keyed case) takes the conservative
/// branch and is blocked when floating — the host cannot prove it targets the active pane.
#[test]
fn source_dependent_without_an_action_is_conservatively_blocked_when_floating() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;
    assert!(!policy_allows(
        session.l(),
        session_domain(session.l()),
        InteractionSource::Keyboard,
        ActionPolicy::SourceDependent,
        None
    ));
}


/// When tiled, sidebar actions are allowed via MouseLeftSidebar.
#[test]
fn tiled_sidebar_action_allowed_via_mouse_sidebar() {
    let session = test_session();
    let actions = [WmAction::SidebarLeft, WmAction::SidebarRight];
    for action in &actions {
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::MouseLeftSidebar,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Allow(_)),
            "Tiled domain should allow {:?} from MouseLeftSidebar, got {:?}",
            action,
            decision,
        );
    }
}


/// When tiled, FocusPane from MouseContent is allowed.
#[test]
fn tiled_content_focus_pane_allowed_via_mouse_content() {
    let session = test_session();
    let decision = route_in_domain(
        session.l(),
        session_domain(session.l()),
        InteractionSource::MouseContent,
        InteractionIntent::ActivateAction(WmAction::FocusPane { pane_id: PaneId(1) }),
    );
    assert!(
        matches!(decision, RouteDecision::Allow(_)),
        "Tiled domain should allow FocusPane from MouseContent, got {:?}",
        decision,
    );
}


// ── E.3: Behavior preservation tests ──

/// When tiled, keyboard navigation actions (FocusLeft/Right/Up/Down) are allowed.
#[test]
fn tiled_keyboard_focus_navigation_allowed() {
    let session = test_session();
    let actions = [
        WmAction::FocusLeft,
        WmAction::FocusRight,
        WmAction::FocusUp,
        WmAction::FocusDown,
    ];
    for action in &actions {
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Allow(_)),
            "Tiled domain should allow {:?} via Keyboard, got {:?}",
            action,
            decision,
        );
    }
}


/// When tiled, workspace actions are allowed.
#[test]
fn tiled_workspace_actions_allowed() {
    let session = test_session();
    let actions = [
        WmAction::WorkspaceNext,
        WmAction::WorkspacePrev,
        WmAction::CreateWorkspace,
    ];
    for action in &actions {
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Allow(_)),
            "Tiled domain should allow {:?} via Keyboard, got {:?}",
            action,
            decision,
        );
    }
}
