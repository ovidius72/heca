//! What the window that asked does with what the server says: the server changes the content
//! alone, and the window shows it, goes where it chooses and focuses what it chooses.

use heca_core::layout::PaneId;
use heca_core::layout::testing::Windowed;

use super::testing::{run_and_react, two_workspaces_of_two_columns};
use crate::input::WmAction;

/// The pane the window shows as active.
fn focused(w: &Windowed) -> Option<PaneId> {
    w.l()
        .active_workspace()
        .and_then(|ws| ws.scroll().active_pane().map(|p| p.id))
}

#[test]
fn a_window_follows_a_pane_it_moved_to_another_workspace() {
    let mut w = two_workspaces_of_two_columns();
    run_and_react(
        &mut w,
        Some(PaneId(1)),
        WmAction::MovePaneToWorkspace {
            pane_id: PaneId(1),
            ws_idx: 1,
        },
    );
    assert_eq!(w.l().active_workspace_idx(), 1);
    assert_eq!(focused(&w), Some(PaneId(1)));
}

#[test]
fn a_window_follows_a_column_it_moved_and_finds_it_active() {
    let mut w = two_workspaces_of_two_columns();
    let action = WmAction::MoveColumnToWorkspace {
        col_idx: 0,
        ws_idx: 1,
        focus: true,
    };
    run_and_react(&mut w, None, action);
    assert_eq!(w.l().active_workspace_idx(), 1);
    assert_eq!(
        focused(&w),
        Some(PaneId(1)),
        "the column it moved is the active one there"
    );
}

/// The workspace before the one shown goes: the window still shows the same workspace, now one
/// number lower.
#[test]
fn a_window_keeps_its_workspace_when_one_before_it_goes() {
    let mut w = two_workspaces_of_two_columns();
    w.show(1);
    let shown = w.session.workspaces[1].id;
    run_and_react(&mut w, None, WmAction::DeleteWorkspace { ws_idx: 0 });
    assert_eq!(w.l().active_workspace_idx(), 0);
    assert_eq!(w.l().active_workspace().map(|ws| ws.id), Some(shown));
}

#[test]
fn a_pane_made_for_the_window_is_focused() {
    let mut w = two_workspaces_of_two_columns();
    run_and_react(&mut w, None, WmAction::AddColumnToWorkspace { ws_idx: 0 });
    let made = w.session.workspaces[0]
        .scrolling
        .columns
        .iter()
        .flat_map(|c| &c.panes)
        .map(|p| p.id)
        .max();
    assert_eq!(focused(&w), made);
}

/// A pane put back from floating is the one the window focuses, where it landed — not the
/// column the window was on.
#[test]
fn an_unfloated_pane_is_focused_where_it_lands() {
    let mut w = Windowed::with_shape(&[&[&[1], &[2], &[3]]]);
    run_and_react(&mut w, Some(PaneId(1)), WmAction::Float);
    assert!(
        w.session.workspaces[0]
            .scrolling
            .pane_indices(PaneId(1))
            .is_none(),
        "it floats"
    );
    w.ws().scroll_mut().activate_column(1);
    assert_eq!(focused(&w), Some(PaneId(3)));
    run_and_react(&mut w, Some(PaneId(1)), WmAction::Float);
    assert_eq!(
        w.session.workspaces[0]
            .scrolling
            .pane_indices(PaneId(1))
            .map(|(c, _)| c),
        Some(0)
    );
    assert_eq!(focused(&w), Some(PaneId(1)));
}
