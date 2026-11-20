//! **The named actions are judged exactly as the things they replaced were.**
//!
//! A click that used to carry its own `InteractionIntent` now names an action. Neither the new name
//! nor the old intent may be judged more or less permissively than the other, so these write down
//! what each was allowed in, domain by domain.

use super::*;

const SOURCE: InteractionSource = InteractionSource::MouseContent;

fn decide(domain: Domain, action: WmAction) -> bool {
    let session = test_session();
    matches!(
        route_in_domain(session.l(), domain, SOURCE, InteractionIntent::ActivateAction(action)),
        RouteDecision::Allow(_)
    )
}

const EVERY_DOMAIN: [(&str, Domain); 5] = [
    ("tiled", Domain::Tiled),
    ("floating", Domain::Floating),
    ("container", Domain::Container),
    ("overlay", Domain::Overlay { terminal_keys: false }),
    ("overlay with a terminal in front", Domain::Overlay { terminal_keys: true }),
];

/// Moving the map's highlight is looking, not acting: allowed everywhere, the map covering the
/// panes included — the only time it ever fires.
#[test]
fn expose_cursor_is_allowed_in_every_domain() {
    for (name, domain) in EVERY_DOMAIN {
        assert!(
            decide(domain, WmAction::ExposeCursor { pane_id: PaneId(3) }),
            "moving the exposé's highlight must work in the {name} domain"
        );
    }
}

/// Folding a named workspace row is refused while a pane floats or something covers the panes, and
/// allowed otherwise — what `ToggleWorkspaceCollapsed` was, and what the same act on the active
/// workspace still is.
#[test]
fn toggling_a_named_workspace_is_judged_as_toggling_the_active_one() {
    for (name, domain) in EVERY_DOMAIN {
        assert_eq!(
            decide(domain, WmAction::ToggleWorkspaceCollapsed { ws_idx: 1 }),
            decide(domain, WmAction::ToggleCurrentWorkspaceCollapsed),
            "{name}: aimed and unaimed must agree"
        );
        let allowed = matches!(domain, Domain::Tiled | Domain::Container);
        assert_eq!(
            decide(domain, WmAction::ToggleWorkspaceCollapsed { ws_idx: 1 }),
            allowed,
            "{name}: the sidebar's fold is for the tiled and container domains only"
        );
    }
}

/// Scrolling a named terminal is judged as scrolling the keyboard's — the same terminal key, aimed.
#[test]
fn scrolling_a_named_terminal_is_judged_as_scrolling_the_keyboards() {
    let named = WmAction::TerminalScroll { pane_id: Some(PaneId(3)), terminal: None, rows: None };
    for (name, domain) in EVERY_DOMAIN {
        assert_eq!(
            decide(domain, named.clone()),
            decide(domain, WmAction::ScrollToOffset { rows: 0 }),
            "{name}: aimed and unaimed must agree"
        );
        assert_eq!(
            decide(domain, named.clone()),
            decide(domain, WmAction::ScrollToBottom),
            "{name}: and so must the way to the bottom"
        );
    }
}
