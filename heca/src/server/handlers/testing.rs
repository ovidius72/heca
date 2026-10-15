//! Fixtures for the tests of the server's handlers.

use heca_core::layout::PaneId;
use heca_core::layout::testing::Windowed;

use crate::input::WmAction;
use crate::server::{Asker, Change, ServerState};

pub(super) fn server() -> ServerState {
    ServerState::for_tests(Default::default())
}

/// A window on a session of two workspaces, each with two columns of one pane: 1, 2 and 11, 12.
pub(super) fn two_workspaces_of_two_columns() -> Windowed {
    Windowed::with_shape(&[&[&[1], &[2]], &[&[11], &[12]]])
}

pub(super) fn run(window: &mut Windowed, action: WmAction) -> Vec<Change> {
    run_for(window, None, action)
}

/// How big the test window draws a terminal cell.
pub(super) const CELL: (f32, f32) = (10.0, 20.0);

/// Run `action` for a window whose focused pane is `focused`, and that shows what it shows.
pub(super) fn run_for(
    window: &mut Windowed,
    focused: Option<PaneId>,
    action: WmAction,
) -> Vec<Change> {
    let asker = Asker::seen_through(window.l(), focused, CELL);
    run_as(window, asker, action)
}

/// Run `action` for this asker — which need not be where the window's own view is, as it is not
/// when two windows share a session.
pub(super) fn run_as(window: &mut Windowed, asker: Asker, action: WmAction) -> Vec<Change> {
    server().run(&mut window.session, asker, &action)
}

/// Run `action` on `server`, for a window whose focused pane is `focused` — for what the server
/// keeps between actions, its terminals.
pub(super) fn run_on(
    server: &mut ServerState,
    window: &mut Windowed,
    focused: Option<PaneId>,
    action: WmAction,
) -> Vec<Change> {
    let asker = Asker::seen_through(window.l(), focused, CELL);
    server.run(&mut window.session, asker, &action)
}

/// The workspaces `changes` say were arranged, each once, in the order they are said.
pub(super) fn arranged(changes: &[Change]) -> Vec<usize> {
    changes
        .iter()
        .filter_map(|change| match change {
            Change::Arranged { workspace, .. } => Some(*workspace),
            _ => None,
        })
        .collect()
}

/// `changes` without what they say was arranged: the facts a window follows.
pub(super) fn facts(changes: &[Change]) -> Vec<Change> {
    changes
        .iter()
        .filter(|change| !matches!(change, Change::Arranged { .. }))
        .cloned()
        .collect()
}

/// Run `action` for the window and have the window react to what it says, as the app does.
pub(super) fn run_and_react(w: &mut Windowed, focused: Option<PaneId>, action: WmAction) {
    let before = w.l().before();
    let changes = run_for(w, focused, action.clone());
    let (mut last_ws, mut panes, mut cursors) = (None, Vec::new(), Vec::new());
    let mut tracking = crate::app_state::reaction::Tracking {
        last_visited_ws: &mut last_ws,
        last_visited_pane_per_ws: &mut panes,
        expose_cursor_per_ws: &mut cursors,
    };
    for change in &changes {
        crate::app_state::reaction::window_reacts(
            &mut w.m(),
            &mut tracking,
            change,
            Some(&action),
            Some(&before),
        );
    }
}
