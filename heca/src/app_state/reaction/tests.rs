//! Tests for [`super`].

use super::*;
use heca_core::layout::testing::Windowed;
use heca_core::layout::PaneId;

/// What a window remembers, owned, for a test to hold.
#[derive(Default)]
struct Tracked {
    last_visited_ws: Option<usize>,
    last_visited_pane_per_ws: Vec<Option<PaneId>>,
    expose_cursor_per_ws: Vec<Option<PaneId>>,
}

impl Tracked {
    fn parts(&mut self) -> Tracking<'_> {
        Tracking {
            last_visited_ws: &mut self.last_visited_ws,
            last_visited_pane_per_ws: &mut self.last_visited_pane_per_ws,
            expose_cursor_per_ws: &mut self.expose_cursor_per_ws,
        }
    }
}

fn two_workspaces_of_two_columns() -> Windowed {
    Windowed::with_shape(&[&[&[1], &[2]], &[&[11], &[12]]])
}

/// A column zoomed in another workspace is shown: the window goes there, remembers where it
/// was, and has that column active.
#[test]
fn a_zoom_elsewhere_shows_that_workspace_and_column() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    let reaction = window_reacts(
        &mut window.m(),
        &mut tracked.parts(),
        &Change::ColumnZoomed { workspace: 1, column: 0 },
        None,
        None,
    );
    assert!(reaction.redraw);
    assert_eq!(window.l().active_workspace_idx(), 1);
    assert_eq!(tracked.last_visited_ws, Some(0));
    let ws = window.l().active_workspace().expect("a workspace");
    assert_eq!(ws.scroll().active_column_idx(), 0);
}

/// Zooming in the workspace already shown does not make it its own "previous" workspace.
#[test]
fn a_zoom_here_does_not_record_a_visit() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    window_reacts(
        &mut window.m(),
        &mut tracked.parts(),
        &Change::ColumnZoomed { workspace: 0, column: 0 },
        None,
        None,
    );
    assert_eq!(tracked.last_visited_ws, None);
}

/// A fact that is not about the layout asks for no redraw of it.
#[test]
fn a_notification_is_not_the_layouts_business() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    let reaction =
        window_reacts(&mut window.m(), &mut tracked.parts(), &Change::NotificationsChanged, None, None);
    assert!(!reaction.redraw);
}

fn react(
    window: &mut Windowed,
    tracked: &mut Tracked,
    change: &Change,
    asked: Option<&WmAction>,
) -> Reaction {
    window_reacts(&mut window.m(), &mut tracked.parts(), change, asked, None)
}

fn landed_in_workspace_1() -> Change {
    Change::PaneMoved { pane: PaneId(1), workspace: 1, column: 0 }
}

/// The window that moved a pane is taken to where it landed; one that only hears of it is not.
#[test]
fn only_the_asking_window_follows_a_moved_pane() {
    let ask = WmAction::MovePaneToWorkspace { pane_id: PaneId(1), ws_idx: 1 };
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    react(&mut window, &mut tracked, &landed_in_workspace_1(), None);
    assert_eq!(window.l().active_workspace_idx(), 0);
    react(&mut window, &mut tracked, &landed_in_workspace_1(), Some(&ask));
    assert_eq!(window.l().active_workspace_idx(), 1);
    assert_eq!(tracked.last_visited_ws, Some(0));
}

/// The window that moved a pane focuses it where it landed; a pane taken with `focus_after` off
/// leaves the focus where it was.
#[test]
fn a_moved_pane_is_focused_unless_the_move_said_not_to() {
    let landed = Change::PaneMoved { pane: PaneId(2), workspace: 0, column: 1 };
    let focused = |window: &Windowed| window.l().active_workspace().and_then(|ws| ws.scroll().active_pane().map(|p| p.id));
    let mut window = two_workspaces_of_two_columns();
    window.m().focus_left();
    let mut tracked = Tracked::default();
    assert_eq!(focused(&window), Some(PaneId(1)));

    let quiet = WmAction::TakePane { pane_id: PaneId(2), focus_after: false };
    react(&mut window, &mut tracked, &landed, Some(&quiet));
    assert_eq!(focused(&window), Some(PaneId(1)));

    react(&mut window, &mut tracked, &landed, Some(&WmAction::MovePaneRight { pane_id: None }));
    assert_eq!(focused(&window), Some(PaneId(2)));
}

/// A column moved with `focus` off stays out of sight; with it on, it is followed.
#[test]
fn a_moved_column_is_followed_only_when_asked_to() {
    let landed = Change::ColumnMoved { workspace: 1, column: 0 };
    let quiet = WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 1, focus: false };
    let loud = WmAction::MoveColumnToWorkspace { col_idx: 0, ws_idx: 1, focus: true };
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    react(&mut window, &mut tracked, &landed, Some(&quiet));
    assert_eq!(window.l().active_workspace_idx(), 0);
    react(&mut window, &mut tracked, &landed, Some(&loud));
    assert_eq!(window.l().active_workspace_idx(), 1);
}

/// A workspace that is gone is forgotten: what pointed at it points at nothing, and what
/// pointed past it moves down one.
#[test]
fn a_removed_workspace_is_forgotten_by_the_window() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked {
        last_visited_ws: Some(2),
        last_visited_pane_per_ws: vec![Some(PaneId(1)), Some(PaneId(2)), Some(PaneId(3))],
        expose_cursor_per_ws: vec![None, Some(PaneId(2)), None],
    };
    react(&mut window, &mut tracked, &Change::WorkspaceRemoved { index: 1 }, None);
    assert_eq!(tracked.last_visited_ws, Some(1));
    assert_eq!(tracked.last_visited_pane_per_ws, [Some(PaneId(1)), Some(PaneId(3))]);
    assert_eq!(tracked.expose_cursor_per_ws, [None, None]);
    react(&mut window, &mut tracked, &Change::WorkspaceRemoved { index: 2 }, None);
    assert_eq!(tracked.last_visited_ws, Some(1), "an earlier one is untouched");
    let mut tracked = Tracked { last_visited_ws: Some(1), ..Tracked::default() };
    react(&mut window, &mut tracked, &Change::WorkspaceRemoved { index: 1 }, None);
    assert_eq!(tracked.last_visited_ws, None, "the one it pointed at is gone");
}

/// A refusal is handed on to be said, and changes nothing the window shows.
#[test]
fn a_refusal_is_handed_on_to_be_said() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    let reaction = react(
        &mut window,
        &mut tracked,
        &Change::Refused(Refusal::OnlyPaneInColumn),
        Some(&WmAction::MovePaneToNewColumn),
    );
    assert_eq!(reaction.refusal, Some(Refusal::OnlyPaneInColumn));
    assert_eq!(window.l().active_workspace_idx(), 0);
    assert!(!refusal_words(Refusal::OnlyPaneInColumn).is_empty());
}

/// A move that changed nothing still takes the asking window to the pane it named.
#[test]
fn a_named_pane_is_focused_even_when_it_did_not_move() {
    let ask = WmAction::MovePaneLeft { pane_id: Some(PaneId(11)) };
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    react(&mut window, &mut tracked, &Change::LayoutChanged, None);
    assert_eq!(window.l().active_workspace_idx(), 0, "another window's change is not followed");
    react(&mut window, &mut tracked, &Change::LayoutChanged, Some(&WmAction::MovePaneLeft { pane_id: None }));
    assert_eq!(window.l().active_workspace_idx(), 0, "no pane named, nothing to follow");
    react(&mut window, &mut tracked, &Change::LayoutChanged, Some(&ask));
    assert_eq!(window.l().active_workspace_idx(), 1);
    assert_eq!(window.l().workspace(1).map(|ws| ws.scroll().active_column_idx()), Some(0));
}

/// Only the asking window is taken to a pane it made; every window is told to start its shell,
/// because until the terminals are behind the server nothing else will. Pinned so that
/// reaction is found and removed with that work: no other fact asks for a shell.
#[test]
fn a_made_pane_gets_its_shell_and_nothing_else_does() {
    let added = Change::PaneAdded { pane: PaneId(7), workspace: 1, column: 0 };
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    let heard = react(&mut window, &mut tracked, &added, None);
    assert_eq!(heard.start_shell, Some((PaneId(7), 1)));
    assert_eq!(window.l().active_workspace_idx(), 0, "another window does not follow");
    let ask = WmAction::AddPaneToColumn { ws_idx: 1, col_idx: 0 };
    react(&mut window, &mut tracked, &added, Some(&ask));
    assert_eq!(window.l().active_workspace_idx(), 1);

    for other in [
        Change::LayoutChanged,
        Change::ColumnZoomed { workspace: 0, column: 0 },
        landed_in_workspace_1(),
        Change::ColumnMoved { workspace: 0, column: 0 },
        Change::WorkspaceRemoved { index: 0 },
        Change::Refused(Refusal::OnlyPaneInColumn),
        Change::NotificationsChanged,
    ] {
        assert_eq!(react(&mut window, &mut tracked, &other, None).start_shell, None, "{other:?}");
    }
}

/// A pane that is gone has its terminal ended by the window, and nothing else asks for that —
/// pinned with the shell start so both go together when the terminals are behind the server.
#[test]
fn a_removed_pane_has_its_terminal_ended_and_nothing_else_does() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    let gone = react(&mut window, &mut tracked, &Change::PaneRemoved { pane: PaneId(2) }, None);
    assert_eq!(gone.stop_terminals, [PaneId(2)]);
    for other in [
        Change::LayoutChanged,
        Change::PaneAdded { pane: PaneId(7), workspace: 0, column: 0 },
        landed_in_workspace_1(),
        Change::WorkspaceRemoved { index: 0 },
        Change::NotificationsChanged,
    ] {
        assert!(react(&mut window, &mut tracked, &other, None).stop_terminals.is_empty(), "{other:?}");
    }
}

/// A name that changed tells the window what shows names is out of date; nothing else does.
#[test]
fn only_a_name_change_says_names_are_out_of_date() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    assert!(react(&mut window, &mut tracked, &Change::NamesChanged, None).names_changed);
    for other in [Change::LayoutChanged, Change::PaneRemoved { pane: PaneId(1) }, landed_in_workspace_1()] {
        assert!(!react(&mut window, &mut tracked, &other, None).names_changed, "{other:?}");
    }
}

/// A change to the columns is shown by the window that asked, from where it had things before —
/// and not by a window with no picture of its own, which has nothing to show it from.
#[test]
fn a_change_to_the_columns_is_shown_from_the_window_s_own_picture() {
    use heca_core::layout::{ColumnEffect, SpaceEffect};
    let mut window = two_workspaces_of_two_columns();
    window.m().focus_right();
    let before = window.l().before();
    let active_before = window.l().workspace(0).map(|ws| ws.scroll().active_column_idx());
    assert_eq!(active_before, Some(1));
    // The content loses its first column; the window that asked from workspace 0 shifts.
    let effect = window
        .m()
        .columns_mut(0)
        .and_then(|space| space.take_column(0))
        .map(|(_, effect)| effect)
        .expect("a column");
    assert_eq!(effect, ColumnEffect::Removed { idx: 0 });
    let mut tracked = Tracked::default();
    let change = Change::Arranged { workspace: 0, effects: vec![SpaceEffect::Column(effect)] };
    window_reacts(&mut window.m(), &mut tracked.parts(), &change, None, None);
    assert_eq!(
        window.l().workspace(0).map(|ws| ws.scroll().active_column_idx()),
        Some(1),
        "a window with no picture shows nothing"
    );
    let reaction = window_reacts(&mut window.m(), &mut tracked.parts(), &change, None, Some(&before));
    assert!(reaction.redraw);
    assert_eq!(
        window.l().workspace(0).map(|ws| ws.scroll().active_column_idx()),
        Some(0),
        "the asking window's active column followed the removal"
    );
}
