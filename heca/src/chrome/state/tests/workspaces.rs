//! The workspaces sidebar's collapse, selection and settings mirrors.

use super::*;

/// A store whose tree holds three workspaces, so a position names one.
fn state_with_three_workspaces() -> SharedChromeState {
    let s = state();
    let session = heca_core::layout::testing::Windowed::with_shape(&[&[&[1]], &[&[2]], &[&[3]]]);
    s.workspaces.tree_mut().sync_from_session(session.l());
    s
}

#[test]
fn toggle_ws_collapsed_flips() {
    let s = state_with_three_workspaces();
    assert!(!s.workspaces.is_ws_collapsed(2));
    s.workspaces.toggle_ws_collapsed(2);
    assert!(s.workspaces.is_ws_collapsed(2));
    s.workspaces.toggle_ws_collapsed(2);
    assert!(!s.workspaces.is_ws_collapsed(2));
}

#[test]
fn set_ws_collapsed_is_idempotent() {
    let s = state_with_three_workspaces();
    s.workspaces.set_ws_collapsed(1, true);
    s.workspaces.set_ws_collapsed(1, true);
    assert!(s.workspaces.is_ws_collapsed(1));
    s.workspaces.set_ws_collapsed(1, false);
    assert!(!s.workspaces.is_ws_collapsed(1));
}

/// **A fold stays on its workspace when the list is reordered.** It is kept by identity, so the
/// workspace that moved is still the folded one at its new position, and the one that took its
/// old position is not.
#[test]
fn a_fold_follows_its_workspace_when_the_list_is_reordered() {
    let s = state();
    let mut session =
        heca_core::layout::testing::Windowed::with_shape(&[&[&[1]], &[&[2]], &[&[3]]]);
    s.workspaces.tree_mut().sync_from_session(session.l());
    s.workspaces.set_ws_collapsed(2, true);
    assert!(session.session.move_workspace(2, 0));
    s.workspaces.tree_mut().sync_from_session(session.l());
    assert!(
        s.workspaces.is_ws_collapsed(0),
        "the moved workspace is still folded"
    );
    assert!(
        !s.workspaces.is_ws_collapsed(2),
        "the one now at its old place is not"
    );
}

#[test]
fn selection_defaults_none_then_set() {
    let s = state();
    assert_eq!(s.workspaces.active_pane(), None);
    s.workspaces.set_active_pane(Some(PaneId(7)));
    assert_eq!(s.workspaces.active_pane(), Some(PaneId(7)));
}

#[test]
fn pane_renamed_add_process_name_defaults_on_then_projects() {
    let s = state();
    // Defaults to the `[settings]` default so the first frame matches config.
    assert!(s.workspaces.pane_renamed_add_process_name());
    s.workspaces.set_pane_renamed_add_process_name(false);
    assert!(!s.workspaces.pane_renamed_add_process_name());
    // Idempotent re-set is a no-op (guarded setter).
    s.workspaces.set_pane_renamed_add_process_name(false);
    assert!(!s.workspaces.pane_renamed_add_process_name());
}

/// **The workspace a window shows is a value to follow and an event to hear**: setting it raises
/// `workspace.focused` once per change, and the signal a dock binds to holds the same key.
#[test]
fn the_shown_workspace_is_followed_and_heard_once_per_change() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let s = state();
    let heard: Rc<RefCell<Vec<String>>> = Rc::default();
    let sink = heard.clone();
    let _sub = s.events().subscribe("workspace.focused", move |event| {
        sink.borrow_mut().push(event.name().to_string())
    });
    let key = |n| {
        crate::entry::service::WorkspaceKey::new(
            crate::entry::service::SessionKey::generate(),
            heca_core::layout::WorkspaceId(n),
        )
    };
    let first = key(3);
    s.workspaces.set_active_workspace(Some(first));
    s.workspaces.set_active_workspace(Some(first));
    assert_eq!(
        heard.borrow().len(),
        1,
        "the same workspace again is no change"
    );
    assert_eq!(
        heca_grid_ui::reactive::SignalGet::get_untracked(&s.workspaces.active_workspace()),
        Some(first)
    );
    s.workspaces.set_active_workspace(Some(key(4)));
    assert_eq!(heard.borrow().len(), 2);
}
