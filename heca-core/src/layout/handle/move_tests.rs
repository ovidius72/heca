//! Swaps and moves of panes and columns, through the layout the way a window moves them.

use crate::layout::testing::Windowed;
use crate::layout::{Moved, Pane, PaneId, Size};

/// A window on `shape`: one workspace per entry, each a list of columns, each a list of pane ids.
fn window(shape: &[&[&[u64]]]) -> Windowed {
    let mut window = Windowed::new(Size::new(1000.0, 800.0), 1.0);
    for _ in 1..shape.len() {
        window.m().add_workspace();
    }
    for (idx, columns) in shape.iter().enumerate() {
        window.show(idx);
        for panes in *columns {
            let mut it = panes.iter();
            let Some(first) = it.next() else { continue };
            window.m().add_pane(Pane::new(PaneId(*first), "p"), None, true);
            let col = window.l().workspace(idx).map_or(0, |ws| ws.scrolling.columns.len() - 1);
            for id in it {
                window.ws().scroll_mut().add_pane_to_column(col, None, Pane::new(PaneId(*id), "p"), false);
            }
        }
    }
    window.show(0);
    window
}

/// The panes of every column of workspace `ws`, as ids.
fn columns(window: &Windowed, ws: usize) -> Vec<Vec<u64>> {
    window.session.workspaces[ws]
        .scrolling
        .columns
        .iter()
        .map(|c| c.panes.iter().map(|p| p.id.0).collect())
        .collect()
}

#[test]
fn two_panes_of_one_column_trade_places() {
    let mut w = window(&[&[&[1, 2, 3]]]);
    assert!(w.m().swap_panes(PaneId(1), PaneId(3)));
    assert_eq!(columns(&w, 0), [[3, 2, 1]]);
}

#[test]
fn two_panes_of_two_columns_trade_places() {
    let mut w = window(&[&[&[1, 2], &[3]]]);
    assert!(w.m().swap_panes(PaneId(2), PaneId(3)));
    assert_eq!(columns(&w, 0), [vec![1, 3], vec![2]]);
}

#[test]
fn two_panes_of_two_workspaces_trade_places() {
    let mut w = window(&[&[&[1], &[2]], &[&[3]]]);
    assert!(w.m().swap_panes(PaneId(1), PaneId(3)));
    assert_eq!(columns(&w, 0), [vec![3], vec![2]]);
    assert_eq!(columns(&w, 1), [vec![1]]);
}

#[test]
fn swapping_a_pane_with_itself_or_a_stranger_changes_nothing() {
    let mut w = window(&[&[&[1], &[2]]]);
    assert!(!w.m().swap_panes(PaneId(1), PaneId(1)));
    assert!(!w.m().swap_panes(PaneId(1), PaneId(99)));
    assert_eq!(columns(&w, 0), [vec![1], vec![2]]);
}

#[test]
fn the_active_pane_swaps_up_and_down() {
    let mut w = window(&[&[&[1, 2, 3]]]);
    if let Some(column) = w.ws().scroll_mut().active_column_mut() {
        column.active_pane_idx = 1;
    }
    assert!(w.ws().swap_active_pane_up());
    assert_eq!(columns(&w, 0), [[2, 1, 3]]);
    assert_eq!(w.session.workspaces[0].scrolling.columns[0].active_pane_idx, 0, "it travelled");
    assert!(!w.ws().swap_active_pane_up(), "already at the top");
}

#[test]
fn a_pane_moves_to_another_column_and_the_emptied_one_goes() {
    let mut w = window(&[&[&[1], &[2], &[3]]]);
    let moved = w.m().move_pane_to_column(PaneId(1), 2);
    assert_eq!(moved, Some(Moved { workspace: 0, column: 1, removed_workspace: None }));
    assert_eq!(columns(&w, 0), [vec![2], vec![3, 1]]);
}

#[test]
fn a_pane_moves_to_a_new_last_column() {
    let mut w = window(&[&[&[1, 2], &[3]]]);
    let moved = w.m().move_pane_to_column(PaneId(1), 2);
    assert_eq!(moved.map(|m| m.column), Some(2));
    assert_eq!(columns(&w, 0), [vec![2], vec![3], vec![1]]);
    assert_eq!(w.m().move_pane_to_column(PaneId(1), 2), None, "already there");
}

#[test]
fn a_pane_alone_in_its_column_does_not_move_to_a_new_one() {
    let mut w = window(&[&[&[1, 2], &[3]]]);
    assert_eq!(w.m().move_pane_to_new_column(PaneId(3)), None);
    let moved = w.m().move_pane_to_new_column(PaneId(1));
    assert_eq!(moved, Some(Moved { workspace: 0, column: 1, removed_workspace: None }));
    assert_eq!(columns(&w, 0), [vec![2], vec![1], vec![3]]);
}

#[test]
fn a_pane_leaving_its_last_place_removes_the_workspace_it_emptied() {
    let mut w = window(&[&[&[1]], &[&[2]], &[&[3]]]);
    let moved = w.m().move_pane_to_workspace(PaneId(1), 2, 0, false);
    assert_eq!(moved, Some(Moved { workspace: 1, column: 0, removed_workspace: Some(0) }));
    assert_eq!(w.session.workspaces.len(), 2);
    assert_eq!(columns(&w, 1), [vec![1], vec![3]]);
}

#[test]
fn a_pane_stacks_into_a_column_of_another_workspace() {
    let mut w = window(&[&[&[1, 9]], &[&[2]]]);
    let moved = w.m().move_pane_to_workspace(PaneId(1), 1, 0, true);
    assert_eq!(moved, Some(Moved { workspace: 1, column: 0, removed_workspace: None }));
    assert_eq!(columns(&w, 1), [vec![2, 1]]);
    assert_eq!(w.m().move_pane_to_workspace(PaneId(1), 1, 0, true), None, "already there");
}

#[test]
fn a_column_moves_to_another_workspace_and_is_active_there() {
    let mut w = window(&[&[&[1], &[2]], &[&[3]]]);
    let moved = w.m().move_column_to_workspace(0, 0, 1);
    assert_eq!(moved, Some(Moved { workspace: 1, column: 1, removed_workspace: None }));
    assert_eq!(columns(&w, 1), [vec![3], vec![1]]);
    assert_eq!(w.l().workspace(1).map(|ws| ws.scroll().active_column_idx()), Some(1));
}

#[test]
fn the_last_column_leaving_removes_its_workspace_and_renumbers_the_target() {
    let mut w = window(&[&[&[1]], &[&[2]]]);
    let moved = w.m().move_column_to_workspace(0, 0, 1);
    assert_eq!(moved, Some(Moved { workspace: 0, column: 1, removed_workspace: Some(0) }));
    assert_eq!(columns(&w, 0), [vec![2], vec![1]]);
    assert_eq!(w.m().move_column_to_workspace(0, 0, 0), None, "same workspace");
}

#[test]
fn a_column_is_reordered_within_its_workspace() {
    let mut w = window(&[&[&[1], &[2], &[3]]]);
    let moved = w.m().move_column(0, 0, 0, 2, false);
    assert_eq!(moved.map(|m| m.column), Some(2));
    assert_eq!(columns(&w, 0), [vec![2], vec![3], vec![1]]);
}

#[test]
fn columns_trade_places_within_and_between_workspaces() {
    let mut w = window(&[&[&[1], &[2]], &[&[3]]]);
    assert!(w.m().swap_columns_between(0, 0, 0, 1));
    assert_eq!(columns(&w, 0), [vec![2], vec![1]]);
    assert!(w.m().swap_columns_between(0, 0, 1, 0));
    assert_eq!(columns(&w, 0), [vec![3], vec![1]]);
    assert_eq!(columns(&w, 1), [vec![2]]);
    assert!(!w.m().swap_columns_between(0, 0, 0, 0), "itself");
    assert!(!w.m().swap_columns_between(0, 5, 1, 0), "out of range");
}

#[test]
fn a_pane_is_activated_in_the_workspace_it_is_in() {
    let mut w = window(&[&[&[1, 4], &[2, 3]]]);
    assert!(w.ws().activate_pane(PaneId(4)));
    assert_eq!(w.l().workspace(0).map(|ws| ws.scroll().active_column_idx()), Some(0));
    assert_eq!(w.session.workspaces[0].scrolling.columns[0].active_pane_idx, 1);
    assert!(!w.ws().activate_pane(PaneId(99)));
}

#[test]
fn the_last_workspace_is_not_removed_when_empty() {
    let mut w = window(&[&[&[1]]]);
    w.ws().scroll_mut().remove_column(0);
    assert!(!w.m().remove_workspace_if_empty(0));
    assert_eq!(w.session.workspaces.len(), 1);
}
