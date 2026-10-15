use super::*;
use heca_core::runtime::{ContentKind, GitInfo, PaneRuntime, ProcessStatus};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn state() -> SharedChromeState {
    SharedChromeState::new(280.0, 260.0)
}
mod pane_runtime;
mod shell;
mod workspaces;

#[test]
fn clone_is_a_shallow_alias_not_a_snapshot() {
    let a = state();
    let b = a.clone();
    a.set_container_scroll("workspaces", 99.0);
    assert_eq!(
        b.container_scroll("workspaces").get_untracked(),
        99.0,
        "clone must alias the same signal store — including the per-mount offsets"
    );
    b.workspaces.toggle_ws_collapsed(3);
    assert!(
        a.workspaces.is_ws_collapsed(3),
        "collapse via clone must be seen by original"
    );
}

#[test]
fn setters_emit_only_on_real_change() {
    let s = state();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_events = seen.clone();
    let _sub = s.events().subscribe("*", move |event| {
        seen_events.borrow_mut().push(event.name().to_string());
    });

    s.workspaces.set_active_pane(Some(PaneId(7)));
    s.workspaces.set_active_pane(Some(PaneId(7)));
    s.set_visible(RegionId::LeftSidebar, false);
    s.set_visible(RegionId::LeftSidebar, false);
    assert_eq!(
        seen.borrow().as_slice(),
        ["pane.active.changed", "chrome.region.visibility.changed"],
    );
}
