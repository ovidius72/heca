//! The workspaces sidebar's collapse, selection and settings mirrors.

use super::*;

#[test]
fn toggle_ws_collapsed_flips() {
    let s = state();
    assert!(!s.workspaces.is_ws_collapsed(2));
    s.workspaces.toggle_ws_collapsed(2);
    assert!(s.workspaces.is_ws_collapsed(2));
    s.workspaces.toggle_ws_collapsed(2);
    assert!(!s.workspaces.is_ws_collapsed(2));
}

#[test]
fn set_ws_collapsed_is_idempotent() {
    let s = state();
    s.workspaces.set_ws_collapsed(1, true);
    s.workspaces.set_ws_collapsed(1, true);
    assert!(s.workspaces.is_ws_collapsed(1));
    s.workspaces.set_ws_collapsed(1, false);
    assert!(!s.workspaces.is_ws_collapsed(1));
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
