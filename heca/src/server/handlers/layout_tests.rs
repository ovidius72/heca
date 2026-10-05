//! The layout actions, run the way the registry runs them: the server's `run` against a session
//! and one window's view of it.

use std::time::Duration;

use heca_core::layout::testing::Windowed;
use heca_core::layout::{ColumnWidth, Pane, PaneId, Size};

use crate::app::backend_store::BackendStore;
use crate::input::{ResizeEdge, ResizeTarget, WmAction};
use crate::notification::NotificationRuntime;
use crate::server::{Asker, Change, ServerCx, ServerState};

fn server() -> ServerState {
    ServerState::new(
        BackendStore::new(),
        NotificationRuntime::with_capacity(
            50,
            Duration::from_millis(4000),
            Default::default(),
            5,
        ),
        Default::default(),
    )
}

/// A window on a session of two workspaces, each with two columns.
fn two_workspaces_of_two_columns() -> Windowed {
    let mut window = Windowed::new(Size::new(1000.0, 800.0), 1.0);
    window.m().add_workspace();
    for idx in 0..2 {
        window.show(idx);
        for n in 0..2u64 {
            window
                .m()
                .add_pane(Pane::new(PaneId(10 * idx as u64 + n + 1), "p"), None, true);
        }
    }
    window.show(0);
    window
}

fn run(window: &mut Windowed, action: WmAction) -> Vec<Change> {
    let mut cx = ServerCx {
        layout: window.m(),
        asker: Asker { focused_pane: None },
    };
    server().run(&mut cx, &action)
}

fn width_of(window: &Windowed, ws: usize, col: usize) -> ColumnWidth {
    window.session.workspaces[ws].scrolling.columns[col].width
}

/// A key that widens the active column by an amount does so, and says the layout changed.
#[test]
fn resizing_the_active_column_changes_its_width() {
    let mut window = two_workspaces_of_two_columns();
    let before = width_of(&window, 0, 1);
    let other = width_of(&window, 0, 0);
    let changes = run(
        &mut window,
        WmAction::Resize {
            target: ResizeTarget::Column,
            amount: -100.0,
            edge: ResizeEdge::Auto,
        },
    );
    assert_eq!(changes, [Change::LayoutChanged]);
    assert_ne!(width_of(&window, 0, 1), before);
    assert_eq!(width_of(&window, 0, 0), other, "the other column is untouched");
}

/// `resize_to` gives the active column a fixed width.
#[test]
fn resizing_a_column_to_a_width_makes_it_fixed() {
    let mut window = two_workspaces_of_two_columns();
    let changes = run(
        &mut window,
        WmAction::ResizeTo {
            target: ResizeTarget::Column,
            width: 333.0,
            height: 0.0,
        },
    );
    assert_eq!(changes, [Change::LayoutChanged]);
    assert_eq!(width_of(&window, 0, 1), ColumnWidth::Fixed(333.0));
}

/// A divider drag resizes the column it names, not the active one.
#[test]
fn a_divider_drag_resizes_the_column_it_names() {
    let mut window = two_workspaces_of_two_columns();
    let before = width_of(&window, 0, 0);
    let changes = run(&mut window, WmAction::ResizeColumnBy { col_idx: 0, delta: 0.1 });
    assert_eq!(changes, [Change::LayoutChanged]);
    assert_ne!(width_of(&window, 0, 0), before);
}

/// A divider drag between stacked panes moves the line between them.
#[test]
fn a_divider_drag_between_stacked_panes_changes_their_heights() {
    let mut window = two_workspaces_of_two_columns();
    window.ws().scroll_mut().add_pane_to_column(0, None, Pane::new(PaneId(99), "x"), false);
    let height = |w: &Windowed| w.session.workspaces[0].scrolling.columns[0].pane_sizes[0].h;
    let before = height(&window);
    let changes = run(
        &mut window,
        WmAction::ResizePaneHeightBy { col_idx: 0, pane_idx: 0, delta: 40.0 },
    );
    assert_eq!(changes, [Change::LayoutChanged]);
    assert_ne!(height(&window), before);
}

/// Zooming a column in a workspace the window is **not** showing zooms that column, moves nothing
/// the window shows, and reports which column it was — the window decides what to show.
#[test]
fn zooming_a_column_elsewhere_changes_content_and_leaves_the_view_alone() {
    let mut window = two_workspaces_of_two_columns();
    let active_before = window.l().active_workspace_idx();
    let changes = run(&mut window, WmAction::ZoomColumnAtIndex { ws_idx: 1, col_idx: 0 });
    assert_eq!(changes, [Change::ColumnZoomed { workspace: 1, column: 0 }]);
    assert!(window.session.workspaces[1].scrolling.columns[0].is_zoomed());
    assert!(!window.session.workspaces[0].scrolling.columns[0].is_zoomed());
    assert_eq!(window.l().active_workspace_idx(), active_before, "the server never switches the shown workspace");
}

/// Naming a workspace or column that is not there changes nothing and says nothing.
#[test]
fn zooming_a_column_that_is_not_there_says_nothing() {
    let mut window = two_workspaces_of_two_columns();
    assert!(run(&mut window, WmAction::ZoomColumnAtIndex { ws_idx: 5, col_idx: 0 }).is_empty());
    assert!(run(&mut window, WmAction::ZoomColumnAtIndex { ws_idx: 0, col_idx: 9 }).is_empty());
}
