use crate::layout::testing::Windowed;
use crate::layout::types::{LayoutOptions, SessionId, Size};
use crate::layout::{Pane, PaneId, Session, WindowView};

fn window() -> Windowed {
    Windowed::new(Size::new(1280.0, 800.0), 2.0)
}

fn session_with_workspaces(count: usize) -> Windowed {
    let mut w = window();
    // `Session::new` creates one workspace; add more if needed.
    for _ in 1..count {
        w.m().add_workspace();
    }
    w
}

#[test]
fn test_remove_workspace_active_after_removed() {
    // 3 workspaces; active is 2. Remove ws 0.
    // active workspace (2) > removed (0) → shift down to 1.
    let mut w = session_with_workspaces(3);
    w.view.active_workspace = 2;
    assert!(w.m().remove_workspace(0));
    assert_eq!(w.session.workspaces.len(), 2);
    assert_eq!(w.view.active_workspace, 1);
}

#[test]
fn test_remove_workspace_active_is_last() {
    // 3 workspaces; active is 2. Remove ws 2 (last).
    // active workspace (2) >= len (2) after removal → clamp to 1.
    let mut w = session_with_workspaces(3);
    w.view.active_workspace = 2;
    assert!(w.m().remove_workspace(2));
    assert_eq!(w.session.workspaces.len(), 2);
    assert_eq!(w.view.active_workspace, 1);
}

#[test]
fn test_remove_workspace_active_before_removed() {
    // 3 workspaces; active is 0. Remove ws 1.
    // active workspace (0) is not > removed (1) and not >= len (2) → unchanged at 0.
    let mut w = session_with_workspaces(3);
    w.view.active_workspace = 0;
    assert!(w.m().remove_workspace(1));
    assert_eq!(w.session.workspaces.len(), 2);
    assert_eq!(w.view.active_workspace, 0);
}

#[test]
fn test_remove_workspace_can_remove_last_leaving_empty() {
    // Removing the sole workspace is allowed; the session is left empty and
    // `active_workspace()` returns `None` (no underflow on the index clamp).
    let mut w = session_with_workspaces(1);
    assert!(w.m().remove_workspace(0));
    assert_eq!(w.session.workspaces.len(), 0);
    assert_eq!(w.view.active_workspace, 0);
    assert!(w.l().active_workspace().is_none());
}

#[test]
fn test_remove_workspace_out_of_bounds() {
    let mut w = session_with_workspaces(2);
    assert!(!w.m().remove_workspace(5));
    assert_eq!(w.session.workspaces.len(), 2);
}

#[test]
fn test_remove_workspace_active_is_removed() {
    // 3 workspaces; active is 1. Remove ws 1.
    // active workspace (1) is not > removed (1) and not >= len (2)
    // → unchanged at 1, which now points to the former workspace 2.
    let mut w = session_with_workspaces(3);
    w.view.active_workspace = 1;
    assert!(w.m().remove_workspace(1));
    assert_eq!(w.session.workspaces.len(), 2);
    assert_eq!(w.view.active_workspace, 1);
}

/// **Two windows on one session move independently** (F012, decision 2c249a29): the content is
/// shared, so a column one window adds is there for both, but where each is looking is its own.
#[test]
fn two_windows_on_one_session_keep_their_own_focus_and_scroll() {
    let mut session = Session::new(SessionId(1), LayoutOptions::default());
    let size = Size::new(1000.0, 800.0);
    let (mut a, mut b) = (WindowView::new(size, 1.0), WindowView::new(size, 1.0));
    for id in 1..=4 {
        session
            .through_mut(&mut a)
            .add_pane(Pane::new(PaneId(id), ""), None, true);
    }
    let first = session.workspaces[0].id;
    assert_eq!(a.scroll(first).active_column, 3, "A added four columns and is on the last");
    assert_eq!(b.scroll(first).active_column, 0, "B has not looked: its view is the blank one");

    // A moves left by one; B moves right by one. Neither sees the other move.
    session.through_mut(&mut a).focus_left();
    session.through_mut(&mut b).focus_right();
    assert_eq!(a.scroll(first).active_column, 2);
    assert_eq!(b.scroll(first).active_column, 1);

    // B reads the content A built.
    assert_eq!(session.through(&b).workspace(0).unwrap().scrolling.columns.len(), 4);
}

/// A window's size is its own: resizing one leaves the other laid out in its old size.
#[test]
fn resizing_one_window_does_not_resize_the_other() {
    let mut session = Session::new(SessionId(1), LayoutOptions::default());
    let size = Size::new(1000.0, 800.0);
    let (mut a, b) = (WindowView::new(size, 1.0), WindowView::new(size, 1.0));
    session
        .through_mut(&mut a)
        .add_pane(Pane::new(PaneId(1), ""), None, true);
    session
        .through_mut(&mut a)
        .update_viewport(Size::new(500.0, 400.0));

    let id = session.workspaces[0].id;
    assert_eq!(a.scroll(id).area.size, Size::new(500.0, 400.0));
    assert_eq!(b.scroll(id).area.size, size);
}

/// **The reads answer for the window that asks**, not for some other: two windows on one session
/// are each told their own focus, through the same `Layout` door.
#[test]
fn a_window_reads_its_own_focus_through_the_layout() {
    let mut session = Session::new(SessionId(1), LayoutOptions::default());
    let size = Size::new(1000.0, 800.0);
    let (mut a, mut b) = (WindowView::new(size, 1.0), WindowView::new(size, 1.0));
    for id in 1..=4 {
        session
            .through_mut(&mut a)
            .add_pane(Pane::new(PaneId(id), ""), None, true);
    }
    session.through_mut(&mut a).focus_left();
    session.through_mut(&mut b).focus_right();

    let focus = |view: &WindowView| {
        session
            .through(view)
            .active_workspace()
            .map(|ws| ws.scroll().active_column_idx())
    };
    assert_eq!(focus(&a), Some(2));
    assert_eq!(focus(&b), Some(1));
}

/// **Resizing the window carries its floating panes along** — they keep their share of the window.
/// The layout has to read each workspace's old area before it sets the new one, so a resize that
/// changed the area first would leave the floats at their old pixel size.
#[test]
fn resizing_the_window_rescales_the_floating_panes() {
    use crate::layout::types::{Point, Rectangle};
    use crate::layout::workspace::FloatingPane;
    let mut w = window();
    w.m().add_pane(Pane::new(PaneId(1), ""), None, true);
    let rect = Rectangle::new(Point::new(100.0, 80.0), Size::new(640.0, 400.0));
    w.ws().add_floating_pane(Pane::new(PaneId(2), ""), rect, None);

    w.m().update_viewport(Size::new(2560.0, 1600.0));

    let float: &FloatingPane = &w.session.workspaces[0].floating_panes[0];
    assert_eq!(float.size, Size::new(1280.0, 800.0), "twice the window, twice the float");
    assert_eq!(float.position, Point::new(200.0, 160.0));
}
