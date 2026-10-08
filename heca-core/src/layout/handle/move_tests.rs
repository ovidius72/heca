//! Swaps and moves of panes and columns, through the layout the way a window moves them.

use crate::layout::testing::Windowed;
use crate::layout::{Moved, Pane, PaneId};

fn window(shape: &[&[&[u64]]]) -> Windowed {
    Windowed::with_shape(shape)
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

#[test]
fn a_pane_is_taken_to_the_bottom_of_the_active_column() {
    let mut w = window(&[&[&[1], &[2, 3]]]);
    let moved = w.m().take_pane_into(PaneId(1), 0, true);
    assert_eq!(moved, Some(Moved { workspace: 0, column: 0, removed_workspace: None }));
    assert_eq!(columns(&w, 0), [vec![2, 3, 1]]);
    assert_eq!(w.m().take_pane_into(PaneId(1), 0, true), None, "already the tail");
    assert_eq!(w.m().take_pane_into(PaneId(99), 0, true), None);
}

#[test]
fn a_floating_pane_is_taken_into_the_tiling() {
    let mut w = window(&[&[&[1]]]);
    let rect = w.ws().reader().default_float_rect();
    w.ws().add_floating_pane(Pane::new(PaneId(9), "f"), rect, None);
    assert!(w.m().take_pane_into(PaneId(9), 0, true).is_some());
    assert_eq!(columns(&w, 0), [[1, 9]]);
    assert!(w.session.workspaces[0].floating_panes.is_empty());
}

#[test]
fn taking_a_floating_pane_leaves_the_others_unfocused() {
    let mut w = window(&[&[&[1]]]);
    let rect = w.ws().reader().default_float_rect();
    w.ws().add_floating_pane(Pane::new(PaneId(8), "f"), rect, None);
    w.ws().add_floating_pane(Pane::new(PaneId(9), "f"), rect, None);
    assert!(w.m().take_pane_into(PaneId(9), 0, true).is_some());
    let ws = &w.session.workspaces[0];
    assert_eq!(ws.floating_panes.len(), 1);
    assert!(!ws.floating_panes[0].is_active);
    assert_eq!(ws.focus_domain, crate::layout::FocusDomain::Tiled);
}

#[test]
fn taking_the_only_pane_of_another_workspace_removes_it_and_renumbers() {
    let mut w = window(&[&[&[1]], &[&[2]]]);
    w.show(1);
    let moved = w.m().take_pane_into(PaneId(1), 1, true);
    assert_eq!(moved, Some(Moved { workspace: 0, column: 0, removed_workspace: Some(0) }));
    assert_eq!(columns(&w, 0), [[2, 1]]);
}

#[test]
fn a_pane_is_placed_at_a_row_or_in_a_new_column() {
    let mut w = window(&[&[&[1], &[2, 3]]]);
    let moved = w.m().place_pane(PaneId(1), 0, 0, Some(1));
    assert_eq!(moved, Some(Moved { workspace: 0, column: 0, removed_workspace: None }));
    assert_eq!(columns(&w, 0), [[2, 1, 3]]);
    let moved = w.m().place_pane(PaneId(3), 0, 0, None);
    assert_eq!(moved.map(|m| m.column), Some(0));
    assert_eq!(columns(&w, 0), [vec![3], vec![2, 1]]);
}

#[test]
fn a_pane_placed_in_a_workspace_that_is_not_there_is_not_lost() {
    let mut w = window(&[&[&[1], &[2]]]);
    assert_eq!(w.m().place_pane(PaneId(1), 5, 0, None), None);
    assert_eq!(columns(&w, 0), [vec![1], vec![2]]);
}

#[test]
fn a_pane_is_made_in_a_column_capped_to_the_last_one() {
    let mut w = window(&[&[&[1], &[2]]]);
    let added = w.m().add_pane_to_column(0, 9).expect("workspace 0");
    assert_eq!((added.workspace, added.column), (0, 1));
    assert_eq!(columns(&w, 0), [vec![1], vec![2, added.pane.0]]);
    assert!(w.m().add_pane_to_column(7, 0).is_none());
}

#[test]
fn a_pane_is_made_in_an_empty_workspace_and_in_a_new_column() {
    let mut w = window(&[&[&[1]], &[]]);
    let first = w.m().add_pane_to_column(1, 3).expect("workspace 1");
    assert_eq!(columns(&w, 1), [vec![first.pane.0]]);
    let second = w.m().add_column(1).expect("workspace 1");
    assert_eq!(columns(&w, 1), [vec![first.pane.0], vec![second.pane.0]]);
    assert_eq!(second.column, 1);
    assert_ne!(first.pane, second.pane);
}

#[test]
fn how_far_a_move_slides_in_from_is_the_layouts_option() {
    let mut w = window(&[&[&[1]]]);
    let full = w.m().slide();
    assert_eq!((full.x, full.y), (900.0, 720.0), "nine tenths of the window by default");
    w.session.options.move_slide_reach = 0.0;
    let none = w.m().slide();
    assert_eq!((none.x, none.y), (0.0, 0.0));
}

#[test]
fn new_options_reach_every_workspace_and_the_ones_made_later() {
    let mut w = window(&[&[&[1]], &[&[2]]]);
    let mut options = w.session.options.clone();
    options.float_size = 0.5;
    w.session.set_options(options);
    assert!(w.session.workspaces.iter().all(|ws| ws.scrolling.options.float_size == 0.5));
    w.m().add_workspace();
    assert_eq!(w.session.workspaces[2].scrolling.options.float_size, 0.5);
    let rect = w.ws().reader().default_float_rect();
    let area = w.l().active_workspace().map(|ws| ws.scroll().area()).expect("a workspace");
    assert_eq!(rect.size.w, area.size.w * 0.5);
}
