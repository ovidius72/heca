//! Tests for [`super`].

use super::*;
use heca_core::layout::PaneId;
use heca_core::layout::testing::Windowed;

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
        &Change::ColumnZoomed {
            workspace: 1,
            column: 0,
        },
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
        &Change::ColumnZoomed {
            workspace: 0,
            column: 0,
        },
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
    let reaction = window_reacts(
        &mut window.m(),
        &mut tracked.parts(),
        &Change::NotificationsChanged,
        None,
        None,
    );
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
    Change::PaneMoved {
        pane: PaneId(1),
        workspace: 1,
        column: 0,
    }
}

/// The window that moved a pane is taken to where it landed; one that only hears of it is not.
#[test]
fn only_the_asking_window_follows_a_moved_pane() {
    let ask = WmAction::MovePaneToWorkspace {
        pane_id: PaneId(1),
        ws_idx: 1,
    };
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    react(&mut window, &mut tracked, &landed_in_workspace_1(), None);
    assert_eq!(window.l().active_workspace_idx(), 0);
    react(
        &mut window,
        &mut tracked,
        &landed_in_workspace_1(),
        Some(&ask),
    );
    assert_eq!(window.l().active_workspace_idx(), 1);
    assert_eq!(tracked.last_visited_ws, Some(0));
}

/// The window that moved a pane focuses it where it landed; a pane taken with `focus_after` off
/// leaves the focus where it was.
#[test]
fn a_moved_pane_is_focused_unless_the_move_said_not_to() {
    let landed = Change::PaneMoved {
        pane: PaneId(2),
        workspace: 0,
        column: 1,
    };
    let mut window = two_workspaces_of_two_columns();
    window.m().focus_left();
    let mut tracked = Tracked::default();
    assert_eq!(focused(&window), Some(PaneId(1)));

    let quiet = WmAction::TakePane {
        pane_id: PaneId(2),
        focus_after: false,
    };
    react(&mut window, &mut tracked, &landed, Some(&quiet));
    assert_eq!(focused(&window), Some(PaneId(1)));

    react(
        &mut window,
        &mut tracked,
        &landed,
        Some(&WmAction::MovePaneRight { pane_id: None }),
    );
    assert_eq!(focused(&window), Some(PaneId(2)));
}

/// A column moved with `focus` off stays out of sight; with it on, it is followed.
#[test]
fn a_moved_column_is_followed_only_when_asked_to() {
    let landed = Change::ColumnMoved {
        workspace: 1,
        column: 0,
    };
    let quiet = WmAction::MoveColumnToWorkspace {
        col_idx: 0,
        ws_idx: 1,
        focus: false,
    };
    let loud = WmAction::MoveColumnToWorkspace {
        col_idx: 0,
        ws_idx: 1,
        focus: true,
    };
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
    react(
        &mut window,
        &mut tracked,
        &Change::WorkspaceRemoved { index: 1 },
        None,
    );
    assert_eq!(tracked.last_visited_ws, Some(1));
    assert_eq!(
        tracked.last_visited_pane_per_ws,
        [Some(PaneId(1)), Some(PaneId(3))]
    );
    assert_eq!(tracked.expose_cursor_per_ws, [None, None]);
    react(
        &mut window,
        &mut tracked,
        &Change::WorkspaceRemoved { index: 2 },
        None,
    );
    assert_eq!(
        tracked.last_visited_ws,
        Some(1),
        "an earlier one is untouched"
    );
    let mut tracked = Tracked {
        last_visited_ws: Some(1),
        ..Tracked::default()
    };
    react(
        &mut window,
        &mut tracked,
        &Change::WorkspaceRemoved { index: 1 },
        None,
    );
    assert_eq!(
        tracked.last_visited_ws, None,
        "the one it pointed at is gone"
    );
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
    let ask = WmAction::MovePaneLeft {
        pane_id: Some(PaneId(11)),
    };
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    react(&mut window, &mut tracked, &Change::LayoutChanged, None);
    assert_eq!(
        window.l().active_workspace_idx(),
        0,
        "another window's change is not followed"
    );
    react(
        &mut window,
        &mut tracked,
        &Change::LayoutChanged,
        Some(&WmAction::MovePaneLeft { pane_id: None }),
    );
    assert_eq!(
        window.l().active_workspace_idx(),
        0,
        "no pane named, nothing to follow"
    );
    react(
        &mut window,
        &mut tracked,
        &Change::LayoutChanged,
        Some(&ask),
    );
    assert_eq!(window.l().active_workspace_idx(), 1);
    assert_eq!(
        window
            .l()
            .workspace(1)
            .map(|ws| ws.scroll().active_column_idx()),
        Some(0)
    );
}

/// The pane the window shows as active.
fn focused(window: &Windowed) -> Option<PaneId> {
    window
        .l()
        .active_workspace()
        .and_then(|ws| ws.scroll().active_pane().map(|p| p.id))
}

/// Only the asking window is taken to a pane it made, and focuses it.
#[test]
fn only_the_asking_window_goes_to_a_pane_it_made() {
    let added = Change::PaneAdded {
        pane: PaneId(12),
        workspace: 1,
        column: 1,
    };
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    react(&mut window, &mut tracked, &added, None);
    assert_eq!(
        window.l().active_workspace_idx(),
        0,
        "another window does not follow"
    );
    let ask = WmAction::AddPaneToColumn {
        ws_idx: 1,
        col_idx: 1,
    };
    react(&mut window, &mut tracked, &added, Some(&ask));
    assert_eq!(window.l().active_workspace_idx(), 1);
    assert_eq!(focused(&window), Some(PaneId(12)));
}

/// A new workspace is known to the window, with nothing visited in it yet.
#[test]
fn a_new_workspace_is_known_with_nothing_visited() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked {
        last_visited_pane_per_ws: vec![Some(PaneId(1)), Some(PaneId(11))],
        expose_cursor_per_ws: vec![None, Some(PaneId(12))],
        ..Tracked::default()
    };
    react(
        &mut window,
        &mut tracked,
        &Change::WorkspaceAdded { index: 2 },
        None,
    );
    assert_eq!(
        tracked.last_visited_pane_per_ws,
        [Some(PaneId(1)), Some(PaneId(11)), None]
    );
    assert_eq!(tracked.expose_cursor_per_ws, [None, Some(PaneId(12)), None]);
}

/// A name that changed tells the window what shows names is out of date; nothing else does.
#[test]
fn only_a_name_change_says_names_are_out_of_date() {
    let mut window = two_workspaces_of_two_columns();
    let mut tracked = Tracked::default();
    assert!(react(&mut window, &mut tracked, &Change::NamesChanged, None).names_changed);
    for other in [
        Change::LayoutChanged,
        Change::PaneRemoved { pane: PaneId(1) },
        landed_in_workspace_1(),
    ] {
        assert!(
            !react(&mut window, &mut tracked, &other, None).names_changed,
            "{other:?}"
        );
    }
}

/// A change to the columns is shown by every window, each keeping its place: the window that
/// asked shows it from where it had things before, and a window with no picture of its own shows
/// it from where things are now.
#[test]
fn a_change_to_the_columns_keeps_each_window_on_its_column() {
    use heca_core::layout::{ColumnEffect, SpaceEffect};
    for asked in [true, false] {
        let mut window = two_workspaces_of_two_columns();
        window.m().focus_right();
        let before = window.l().before();
        let active_before = window
            .l()
            .workspace(0)
            .map(|ws| ws.scroll().active_column_idx());
        assert_eq!(active_before, Some(1));
        // The content loses its first column, which renumbers the one the window is on.
        let effect = window
            .m()
            .columns_mut(0)
            .and_then(|space| space.take_column(0))
            .map(|(_, effect)| effect)
            .expect("a column");
        assert_eq!(effect, ColumnEffect::Removed { idx: 0 });
        let mut tracked = Tracked::default();
        let change = Change::Arranged {
            workspace: 0,
            effects: vec![SpaceEffect::Column(effect)],
        };
        let picture = asked.then_some(&before);
        let reaction = window_reacts(
            &mut window.m(),
            &mut tracked.parts(),
            &change,
            None,
            picture,
        );
        assert!(reaction.redraw);
        assert_eq!(
            window
                .l()
                .workspace(0)
                .map(|ws| ws.scroll().active_column_idx()),
            Some(0),
            "with a picture of its own: {asked}; still on the column it was on"
        );
    }
}

/// A floating pane spawned as content in workspace 0.
fn with_a_float(pane: u64) -> Windowed {
    let mut window = two_workspaces_of_two_columns();
    window.session.workspaces[0]
        .add_floating_pane(heca_core::layout::Pane::new(PaneId(pane), "f"), None);
    window
}

/// The rect the window draws a float at, and whether it is the window's active float.
fn float_in(window: &Windowed, pane: u64) -> Option<(heca_core::layout::Rectangle, bool)> {
    window
        .l()
        .workspace(0)
        .and_then(|ws| {
            ws.floating()
                .into_iter()
                .find(|f| f.pane.id == PaneId(pane))
                .map(|f| (f.rect, f.active))
        })
}

/// The window that floats a pane places it where it drew it and focuses the floats.
#[test]
fn the_asking_window_places_and_focuses_a_float() {
    let mut window = with_a_float(7);
    let mut tracked = Tracked::default();
    let rect = heca_core::layout::Rectangle::new(
        heca_core::layout::Point::new(40.0, 30.0),
        heca_core::layout::Size::new(200.0, 150.0),
    );
    let ask = WmAction::Float;
    react(
        &mut window,
        &mut tracked,
        &Change::Floated {
            pane: PaneId(7),
            workspace: 0,
            rect,
        },
        Some(&ask),
    );
    let ws = window.l().active_workspace().expect("a workspace");
    assert_eq!(ws.focus_domain(), heca_core::layout::FocusDomain::Floating);
    assert_eq!(ws.active_float(), Some(PaneId(7)));
    assert_eq!(float_in(&window, 7), Some((rect, true)));
}

/// A window that only hears a float appeared keeps its own view: it stays on the tiling and draws
/// the float at its own centred default, not at the asking window's rect.
#[test]
fn a_bystanding_window_keeps_its_view_when_a_float_appears() {
    let mut window = with_a_float(7);
    let mut tracked = Tracked::default();
    let rect = heca_core::layout::Rectangle::new(
        heca_core::layout::Point::new(40.0, 30.0),
        heca_core::layout::Size::new(200.0, 150.0),
    );
    react(
        &mut window,
        &mut tracked,
        &Change::Floated {
            pane: PaneId(7),
            workspace: 0,
            rect,
        },
        None,
    );
    let ws = window.l().active_workspace().expect("a workspace");
    assert_eq!(ws.focus_domain(), heca_core::layout::FocusDomain::Tiled);
    assert_eq!(ws.active_float(), None);
    let (drawn, active) = float_in(&window, 7).expect("the float is still content");
    assert_ne!(drawn, rect, "it falls back to its own centred default");
    assert!(!active);
}

/// Closing the only float returns the window to the tiling when it reacts to the removal.
#[test]
fn closing_the_last_float_returns_the_window_to_the_tiling() {
    let mut window = with_a_float(7);
    let mut tracked = Tracked::default();
    let rect = heca_core::layout::Rectangle::new(
        heca_core::layout::Point::new(40.0, 30.0),
        heca_core::layout::Size::new(200.0, 150.0),
    );
    let ask = WmAction::Float;
    react(
        &mut window,
        &mut tracked,
        &Change::Floated {
            pane: PaneId(7),
            workspace: 0,
            rect,
        },
        Some(&ask),
    );
    // Content loses the float, then the window reacts to the removal.
    window.session.workspaces[0]
        .floating_panes
        .retain(|f| f.pane.id != PaneId(7));
    react(&mut window, &mut tracked, &Change::PaneRemoved { pane: PaneId(7) }, None);
    let ws = window.l().active_workspace().expect("a workspace");
    assert_eq!(ws.focus_domain(), heca_core::layout::FocusDomain::Tiled);
    assert_eq!(ws.active_float(), None);
}
