use super::{DockFocus, dock_focus_outcome};

/// **`FocusDock` only focuses.** Asking to focus the dock that already has the keyboard does
/// nothing — it does not hand it back.
///
/// The toggle lived inside `FocusDock` until F003/P082/T444, so *everything* that asked to focus
/// a dock inherited it: a click inside a focused dock released it, and so did an RPC
/// `focus-dock` and the command palette. `aim_keyboard_at_click` carried an `if` to work around
/// it, which is a rule in a call site rather than in the model.
#[test]
fn focusing_a_dock_that_already_has_the_keyboard_does_nothing() {
    assert_eq!(
        dock_focus_outcome(Some("workspaces"), "workspaces", false),
        DockFocus::Nothing
    );
}

/// **`ToggleDock` hands it back** — `prefix+e` in, `prefix+e` out. The toggle belongs to the
/// gesture: pressing a key again plainly means "undo that", while a click never does.
#[test]
fn toggling_a_dock_that_already_has_the_keyboard_gives_it_back() {
    assert_eq!(
        dock_focus_outcome(Some("workspaces"), "workspaces", true),
        DockFocus::Release
    );
}

/// Both take it when the dock does not have it — that half is the same gesture either way.
#[test]
fn either_way_a_dock_without_the_keyboard_takes_it() {
    assert_eq!(
        dock_focus_outcome(None, "workspaces", false),
        DockFocus::Take
    );
    assert_eq!(
        dock_focus_outcome(None, "workspaces", true),
        DockFocus::Take
    );
    assert_eq!(
        dock_focus_outcome(Some("notes"), "workspaces", false),
        DockFocus::Take
    );
    assert_eq!(
        dock_focus_outcome(Some("notes"), "workspaces", true),
        DockFocus::Take
    );
}
