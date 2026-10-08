//! What a floating pane blocks and allows, from each source.

use super::*;


/// In Floating domain, TiledOnly actions are blocked.
#[test]
fn floating_domain_blocks_tiled_only() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    let actions = [
        WmAction::FocusLeft,
        WmAction::FocusRight,
        WmAction::FocusUp,
        WmAction::FocusDown,
        WmAction::SplitHorizontal,
        WmAction::ZoomColumn,
        WmAction::SidebarLeft,
        WmAction::WorkspaceNext,
        WmAction::CommandPalette {
            mode: None,
            query: None,
        },
        WmAction::FocusToggleLocal,
        WmAction::PaneSelect,
        WmAction::FloatAt {
            pane_id: PaneId(0),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    ];
    for action in &actions {
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "Floating domain should block {:?} but got {:?}",
            action,
            decision,
        );
    }
}


/// In Floating domain, FocusedPaneLocal actions are allowed.
#[test]
fn floating_domain_allows_focused_pane_local() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    let actions = [
        WmAction::Float,
        WmAction::ClosePane,
        WmAction::RenamePane,
        WmAction::OpenContextMenu,
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
            "Floating domain should allow {:?} but got {:?}",
            action,
            decision,
        );
    }
}


/// Intent variants are blocked when floating.
#[test]
fn floating_domain_blocks_intent_variants() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    let intents = [
        InteractionIntent::FocusPane {
            pane_id: PaneId(99),
        },
        InteractionIntent::StartSidebarDrag {
            pane_id: PaneId(42),
        },
    ];
    for intent in &intents {
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::Keyboard,
            intent.clone(),
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "Floating domain should block {:?} but got {:?}",
            intent,
            decision,
        );
    }
}


/// MouseContent FocusPane is blocked when floating.
#[test]
fn floating_blocks_mouse_content_focus_pane() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    // MouseContent FocusPane should be blocked when floating
    // (only the active floating pane could receive focus, and this targets a tiled pane)
    let decision = route_in_domain(
        session.l(),
        session_domain(session.l()),
        InteractionSource::MouseContent,
        InteractionIntent::ActivateAction(WmAction::FocusPane {
            pane_id: PaneId(99),
        }),
    );
    assert!(
        matches!(decision, RouteDecision::Block),
        "MouseContent FocusPane should be blocked when floating, got {:?}",
        decision,
    );
}


/// MouseLeftSidebar actions are blocked when floating (via the guard in mouse.rs,
/// tested here as part of the router's policy).
#[test]
fn floating_blocks_mouse_left_sidebar_actions() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    // Sidebar actions should be blocked when floating.
    let actions = [WmAction::SidebarLeft];
    for action in &actions {
        // Test via MouseLeftSidebar source (same result as Keyboard, but testing the source explicitly)
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::MouseLeftSidebar,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "MouseLeftSidebar should block {:?} when floating, got {:?}",
            action,
            decision,
        );
    }
}


// ═══════════════════════════════════════════════════════════════════════
// Phase E — Regression tests
// ═══════════════════════════════════════════════════════════════════════

// ── E.2: Floating-domain focus blocking regression tests ──

/// When floating, keyboard FocusPane targeting the active floating pane
/// is allowed (SourceDependent policy).
#[test]
fn floating_focus_allows_active_floating_pane_via_keyboard() {
    let mut session = test_session();
    session.m().add_pane(Pane::new(PaneId(99), "float-99"), None, true);
    let mut ws = session.ws();
    ws.add_floating_pane(
        Pane::new(PaneId(99), "float-99"),
        Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 100.0)),
        None,
    );
    ws.focus_domain = FocusDomain::Floating;

    // FocusPane targeting the active floating pane should be allowed from Keyboard.
    let decision = route_in_domain(
        session.l(),
        session_domain(session.l()),
        InteractionSource::Keyboard,
        InteractionIntent::ActivateAction(WmAction::FocusPane {
            pane_id: PaneId(99),
        }),
    );
    assert!(
        matches!(decision, RouteDecision::Allow(_)),
        "FocusPane on active floating pane should be allowed when floating, got {:?}",
        decision,
    );
}


/// When floating, keyboard FocusPane targeting a tiled pane is blocked.
#[test]
fn floating_focus_blocks_tiled_pane_via_keyboard() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    // FocusPane targeting tiled pane (ID 1) should be blocked.
    let decision = route_in_domain(
        session.l(),
        session_domain(session.l()),
        InteractionSource::Keyboard,
        InteractionIntent::ActivateAction(WmAction::FocusPane { pane_id: PaneId(1) }),
    );
    assert!(
        matches!(decision, RouteDecision::Block),
        "FocusPane on tiled pane should be blocked when floating, got {:?}",
        decision,
    );
}


/// When floating, FocusPane intent from MouseContent is blocked.
#[test]
fn floating_focus_pane_intent_blocked_via_mouse_content() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    let decision = route_in_domain(
        session.l(),
        session_domain(session.l()),
        InteractionSource::MouseContent,
        InteractionIntent::FocusPane { pane_id: PaneId(1) },
    );
    assert!(
        matches!(decision, RouteDecision::Block),
        "FocusPane intent from MouseContent should be blocked when floating, got {:?}",
        decision,
    );
}


/// When floating, AlwaysAllowed actions (CommandPalette, SpawnCommand) are
/// blocked from current UI sources. ReloadConfig is NOT in this group — it
/// is `Global` (always allowed, even when floating) because it has no layout
/// impact; see `floating_allows_global_reload`.
#[test]
fn floating_blocks_always_allowed_from_keyboard() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    let actions = [
        WmAction::CommandPalette {
            mode: None,
            query: None,
        },
        WmAction::SpawnCommand {
            command: String::new(),
            kind: crate::input::SpawnKind::Terminal,
            float: false,
            close_policy: heca_core::runtime::PaneClosePolicy::default(),
            cwd: None,
        },
    ];
    for action in &actions {
        let decision = route_in_domain(
            session.l(),
            session_domain(session.l()),
            InteractionSource::Keyboard,
            InteractionIntent::ActivateAction(action.clone()),
        );
        assert!(
            matches!(decision, RouteDecision::Block),
            "Floating domain should block {:?} from Keyboard (may allow from chrome later), got {:?}",
            action,
            decision,
        );
    }
}


/// ReloadConfig is a `Global` action — allowed in every focus domain,
/// including Floating (it reloads config from disk with no layout impact).
/// Regression test for the bug where hot-reload silently failed whenever a
/// floating pane was active (style only applied on full restart).
#[test]
fn floating_allows_global_reload() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    let decision = route_in_domain(
        session.l(),
        session_domain(session.l()),
        InteractionSource::Keyboard,
        InteractionIntent::ActivateAction(WmAction::ReloadConfig),
    );
    assert!(
        matches!(
            decision,
            RouteDecision::Allow(InteractionIntent::ActivateAction(_))
        ),
        "Floating domain should allow ReloadConfig from Keyboard (Global action), got {:?}",
        decision,
    );
}


/// When floating, FocusedPaneLocal actions (Float, ClosePane, RenamePane,
/// scrollback, selection actions) are still allowed.
#[test]
fn floating_allows_focused_pane_local_via_keyboard() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    let actions = [
        WmAction::Float,
        WmAction::ClosePane,
        WmAction::RenamePane,
        // scrollback
        WmAction::ScrollbackPageUp,
        WmAction::ScrollbackPageDown,
        WmAction::ScrollbackToTop,
        WmAction::ScrollbackToBottom,
        WmAction::ExitScrollback,
        // Direct scroll (same policy)
        WmAction::ScrollLineUp,
        WmAction::ScrollLineDown,
        WmAction::ScrollPageUp,
        WmAction::ScrollPageDown,
        WmAction::ScrollToTop,
        WmAction::ScrollToBottom,
        // Selection (host capability, Task 02) — allowed in both domains.
        WmAction::EnterSelectionMode,
        WmAction::SelectionLeft,
        WmAction::SelectionRight,
        WmAction::SelectionUp,
        WmAction::SelectionDown,
        WmAction::ClearSelection,
        WmAction::CopySelection,
        WmAction::PasteClipboard,
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
            "Floating domain should allow {:?} via Keyboard, got {:?}",
            action,
            decision,
        );
    }
}


/// When floating, FocusedPaneLocal actions are allowed from all sources.
#[test]
fn floating_allows_focused_pane_local_from_all_sources() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;

    let sources = [
        InteractionSource::Keyboard,
        InteractionSource::MouseContent,
        InteractionSource::MouseLeftSidebar,
    ];
    let actions = [
        WmAction::Float,
        WmAction::ClosePane,
        // Selection (host capability, Task 02) — allowed from all sources.
        WmAction::EnterSelectionMode,
        WmAction::SelectionLeft,
        WmAction::SelectionRight,
        WmAction::SelectionUp,
        WmAction::SelectionDown,
        WmAction::ClearSelection,
        WmAction::CopySelection,
        WmAction::PasteClipboard,
    ];

    for source in &sources {
        for action in &actions {
            let decision = route_in_domain(
                session.l(),
                session_domain(session.l()),
                *source,
                InteractionIntent::ActivateAction(action.clone()),
            );
            assert!(
                matches!(decision, RouteDecision::Allow(_)),
                "Floating domain should allow {:?} from {:?}, got {:?}",
                action,
                source,
                decision,
            );
        }
    }
}
