//! Which domain the screen is in, and which pane can take the keyboard in it.

use super::*;


// ── Focus-target helper tests ──

/// `active_focus_domain` returns Tiled for default workspace.
#[test]
fn active_focus_domain_default_is_tiled() {
    let session = test_session();
    assert_eq!(active_focus_domain(session.l()), FocusDomain::Tiled);
}


/// `active_focus_domain` returns Floating after setting.
#[test]
fn active_focus_domain_floating_after_set() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;
    assert_eq!(active_focus_domain(session.l()), FocusDomain::Floating);
}


/// `can_focus_pane` allows any pane when in tiled domain.
#[test]
fn can_focus_pane_allows_any_in_tiled_domain() {
    let session = test_session();
    assert!(can_focus_pane(
        session.l(),
        InteractionSource::Keyboard,
        PaneId(1)
    ));
    assert!(can_focus_pane(
        session.l(),
        InteractionSource::MouseContent,
        PaneId(1)
    ));
    assert!(can_focus_pane(
        session.l(),
        InteractionSource::MouseLeftSidebar,
        PaneId(1)
    ));
}


/// `can_focus_pane` blocks non-floating pane when in floating domain.
#[test]
fn can_focus_pane_blocks_non_floating_when_floating() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;
    // In a floating domain, pane 1 (in scrolling columns) is NOT the active floating pane.
    // So can_focus_pane should block it from all sources.
    assert!(!can_focus_pane(
        session.l(),
        InteractionSource::Keyboard,
        PaneId(1)
    ));
    assert!(!can_focus_pane(
        session.l(),
        InteractionSource::MouseContent,
        PaneId(1)
    ));
    assert!(!can_focus_pane(
        session.l(),
        InteractionSource::MouseLeftSidebar,
        PaneId(1)
    ));
}


/// Sidebar navigation intent is blocked when floating.
/// Sidebar navigation intent is allowed when tiled.
/// can_focus_pane allows the active floating pane when in floating domain.
#[test]
fn can_focus_pane_allows_active_floating_pane() {
    let mut session = test_session();
    // Add a floating pane with ID 99, set active
    let mut ws = session.ws();
    ws.update_working_area(Rectangle::new(
        Point::new(0.0, 0.0),
        Size::new(1280.0, 800.0),
    ));
    ws.add_floating_pane(
        Pane::new(PaneId(99), "float-99"),
        Rectangle::new(Point::new(50.0, 50.0), Size::new(800.0, 600.0)),
        None,
    );
    ws.focus_domain = FocusDomain::Floating;

    // The active floating pane (ID 99) can be focused from all sources.
    assert!(can_focus_pane(
        session.l(),
        InteractionSource::Keyboard,
        PaneId(99)
    ));
    assert!(can_focus_pane(
        session.l(),
        InteractionSource::MouseContent,
        PaneId(99)
    ));
    assert!(can_focus_pane(
        session.l(),
        InteractionSource::MouseLeftSidebar,
        PaneId(99)
    ));
}


/// is_floating_domain returns false for default (Tiled) workspace.
#[test]
fn is_floating_domain_default_is_tiled() {
    let session = test_session();
    assert!(!is_floating_domain(session.l()));
}


/// Setting focus_domain to Floating is detected by helpers.
#[test]
fn floating_domain_detected_after_set() {
    let mut session = test_session();
    session.ws().focus_domain = FocusDomain::Floating;
    assert!(is_floating_domain(session.l()));
}
