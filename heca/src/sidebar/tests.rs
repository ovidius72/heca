#![allow(clippy::module_inception)]

    use super::*;
    use crate::app_state::SidebarItemState;
    use heca_core::layout::{
        Pane as LayoutPane, PaneId,
        session::Session,
        types::{SessionId, Size},
    };

    // Workspace collapse is owned by chrome_state; these helpers drive it the way the
    // app does — mutate chrome_state, then project onto the tree via apply_ws_collapsed.
    fn test_chrome() -> crate::chrome::SharedChromeState {
        crate::chrome::SharedChromeState::new(200.0, true, 200.0, true)
    }
    fn toggle_ws(tree: &mut SidebarTree, chrome: &crate::chrome::SharedChromeState, ws_idx: usize) {
        chrome.workspaces.toggle_ws_collapsed(ws_idx);
        let set = chrome.workspaces.with_collapsed_ws(|s| s.clone());
        tree.apply_ws_collapsed(&set, Some(ws_idx));
    }
    fn set_ws_collapsed(tree: &mut SidebarTree, chrome: &crate::chrome::SharedChromeState, ws_idx: usize, collapsed: bool) {
        chrome.workspaces.set_ws_collapsed(ws_idx, collapsed);
        let set = chrome.workspaces.with_collapsed_ws(|s| s.clone());
        tree.apply_ws_collapsed(&set, Some(ws_idx));
    }

    fn make_test_session() -> (Session, Vec<u64>) {
        let viewport = Size::new(1280.0, 800.0);
        let mut session = Session::new(
            SessionId(1),
            viewport,
            2.0,
            heca_core::layout::types::LayoutOptions::default(),
        );

        // Create 3 panes in the first workspace
        let _ids: Vec<u64> = (1..=4)
            .map(|i| {
                let pane = LayoutPane::new(PaneId(i), format!("Pane{}", i));
                let id = pane.id.0;
                session.add_pane(pane, None, true);
                id
            })
            .collect();

        // Add a second workspace with 1 pane
        let wa = session
            .active_workspace()
            .map(|ws| {
                let r = ws.scrolling.working_area;
                heca_core::layout::types::Rectangle::new(r.loc, r.size)
            })
            .unwrap_or_else(|| {
                heca_core::layout::types::Rectangle::new(
                    heca_core::layout::types::Point::new(0.0, 0.0),
                    viewport,
                )
            });
        session.add_workspace(wa);
        let pane5 = LayoutPane::new(PaneId(5), "Pane5");
        let _id5 = pane5.id.0;
        session.add_pane(pane5, None, true);

        (session, vec![1, 2, 3, 4, 5])
    }

    #[test]
    fn test_tree_rebuild() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();

        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Should have 2 workspaces
        assert_eq!(tree.workspaces.len(), 2, "should have 2 workspaces");

        // WS 0 should be active (the one with panes)
        assert_eq!(
            tree.workspaces[0].state,
            SidebarItemState::Active,
            "WS 0 should be active"
        );
        assert_eq!(
            tree.workspaces[1].state,
            SidebarItemState::None,
            "WS 1 should be none (not yet visited)"
        );

        // WS 0 should have some columns with panes
        let ws0 = &tree.workspaces[0];
        assert!(!ws0.columns.is_empty(), "WS 0 should have columns");
        assert!(!ws0.collapsed, "WS 0 should not be collapsed by default");
    }

    #[test]
    fn test_tree_flat_items() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Flat items should contain workspaces, columns, and panes
        assert!(
            !tree.flat_items.is_empty(),
            "flat items should not be empty"
        );

        // First item should be a workspace
        match &tree.flat_items[0] {
            SidebarItem::Workspace { .. } => {}
            other => panic!("first flat item should be Workspace, got {:?}", other),
        }

        // Item count should match flat_items.len()
        assert_eq!(
            tree.item_count,
            tree.flat_items.len(),
            "item_count should match"
        );
    }

    #[test]
    fn test_cursor_movement() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        assert_eq!(tree.cursor, 0, "cursor starts at 0");

        tree.cursor_down();
        assert_eq!(tree.cursor, 1, "cursor moves down to 1");

        tree.cursor_up();
        assert_eq!(tree.cursor, 0, "cursor moves up back to 0");

        // Move to end, then past end should clamp
        for _ in 0..tree.item_count + 5 {
            tree.cursor_down();
        }
        assert_eq!(
            tree.cursor,
            tree.item_count - 1,
            "cursor clamps at last item"
        );

        // Move past start should clamp at 0
        for _ in 0..103 {
            tree.cursor_up();
        }
        assert_eq!(tree.cursor, 0, "cursor clamps at first item");
    }

    #[test]
    fn test_expand_collapse_workspace() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Initially not collapsed
        assert!(
            !tree.workspaces[0].collapsed,
            "WS 0 should not be collapsed initially"
        );

        // Move cursor to workspace 0 and toggle expand
        let chrome = test_chrome();
        tree.cursor = 0;
        tree.toggle_expand(&chrome.workspaces);

        // Should now be collapsed
        assert!(
            tree.workspaces[0].collapsed,
            "WS 0 should be collapsed after toggle"
        );

        // Flat items should have fewer items (children hidden)
        let collapsed_count = tree.flat_items.len();

        // Toggle again to expand
        tree.toggle_expand(&chrome.workspaces);
        assert!(
            !tree.workspaces[0].collapsed,
            "WS 0 should be expanded after second toggle"
        );
        assert!(
            tree.flat_items.len() > collapsed_count,
            "flat items should increase after expand"
        );
    }

    #[test]
    fn test_visited_tracking() {
        let (mut session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();

        // Simulate visiting workspace 1 (switch to it)
        session.switch_to_workspace(1);

        // Rebuild with last_visited_ws_idx = 0 (WS 0 was visited before)
        tree.sync_from_session(&session, Some(0), Some(PaneId(5)), &[]);

        // WS 1 should be active (current)
        assert_eq!(
            tree.workspaces[1].state,
            SidebarItemState::Active,
            "WS 1 should be active after switch"
        );
        // WS 0 should be visited
        assert_eq!(
            tree.workspaces[0].state,
            SidebarItemState::Visited,
            "WS 0 should be visited"
        );
        // WS 2 (doesn't exist) is none
    }

    #[test]
    fn test_rebuild_clears_previous() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();

        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);
        let first_count = tree.flat_items.len();

        // Rebuild again — should be same result
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);
        assert_eq!(
            tree.flat_items.len(),
            first_count,
            "rebuild should produce same result"
        );

        // Cursor should be clamped if it was out of bounds
        tree.cursor = 9999;
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);
        assert!(
            tree.cursor < tree.flat_items.len(),
            "cursor should be clamped after rebuild"
        );
    }

    #[test]
    fn test_empty_session() {
        let viewport = Size::new(1280.0, 800.0);
        let session = Session::new(
            SessionId(1),
            viewport,
            2.0,
            heca_core::layout::types::LayoutOptions::default(),
        );
        let mut tree = SidebarTree::new();

        tree.sync_from_session(&session, None, None, &[]);

        // Even an empty session has at least 1 workspace (the initial one)
        assert!(
            !tree.workspaces.is_empty(),
            "should have at least 1 workspace"
        );
        // But it may have no panes
    }

    #[test]
    fn test_cursor_down_collapsed_skips_columns() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Start at first workspace (index 0). In collapsed mode cursor_down
        // should skip the Column item and land on the first Pane.
        tree.cursor = 0;
        tree.cursor_down_collapsed();
        assert!(
            matches!(tree.flat_items[tree.cursor], SidebarItem::Pane { .. }),
            "cursor_down_collapsed should skip Column and land on Pane, got {:?}",
            tree.flat_items[tree.cursor]
        );
    }

    #[test]
    fn test_cursor_up_collapsed_skips_columns() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Put cursor on the first Pane item after its Column.
        // cursor_up_collapsed should skip the Column and land on Workspace.
        let first_pane_idx = tree
            .flat_items
            .iter()
            .position(|i| matches!(i, SidebarItem::Pane { .. }))
            .expect("should have a pane");
        tree.cursor = first_pane_idx;
        tree.cursor_up_collapsed();
        assert!(
            matches!(tree.flat_items[tree.cursor], SidebarItem::Workspace { .. }),
            "cursor_up_collapsed should skip Column and land on Workspace, got {:?}",
            tree.flat_items[tree.cursor]
        );
    }

    #[test]
    fn test_toggle_expand_clamps_cursor() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Place cursor deep inside workspace 0 (e.g. on a pane).
        let ws0_last_idx = tree
            .flat_items
            .iter()
            .enumerate()
            .rposition(|(_, i)| {
                matches!(i, SidebarItem::Workspace { ws_idx } if *ws_idx == 0)
                    || matches!(i, SidebarItem::Column { ws_idx, .. } if *ws_idx == 0)
                    || matches!(i, SidebarItem::Pane { pane_id } if pane_id.0 <= 4)
            })
            .expect("should have ws0 items");
        tree.cursor = ws0_last_idx;

        // Collapse workspace 0 — its children disappear.
        tree.workspaces[0].collapsed = true;
        tree.sync_flat_items();
        tree.clamp_cursor();

        // clamp_cursor() only bounds-checks; cursor may end up on a later workspace.
        // The invariant is that cursor must be valid after collapse.
        assert!(
            tree.cursor < tree.flat_items.len(),
            "cursor must be valid after collapse: cursor={} len={}",
            tree.cursor,
            tree.flat_items.len()
        );
    }

    #[test]
    fn test_column_expand_collapse() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Find first Column item in flat list.
        let col_idx = tree
            .flat_items
            .iter()
            .position(|i| matches!(i, SidebarItem::Column { .. }))
            .expect("should have a column");
        tree.cursor = col_idx;
        let chrome = test_chrome();

        // Collapse the column.
        tree.toggle_expand(&chrome.workspaces);
        if let SidebarItem::Column { ws_idx, col_idx: c } = tree.flat_items[tree.cursor] {
            assert!(
                tree.workspaces[ws_idx].columns[c].collapsed,
                "column should be collapsed"
            );
        }

        // Expand it back.
        tree.toggle_expand(&chrome.workspaces);
        if let SidebarItem::Column { ws_idx, col_idx: c } = tree.flat_items[tree.cursor] {
            assert!(
                !tree.workspaces[ws_idx].columns[c].collapsed,
                "column should be expanded"
            );
        }
    }

    #[test]
    fn test_collapse_workspace_moves_cursor_to_workspace_row() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        let pane_idx = tree
            .flat_items
            .iter()
            .position(|item| matches!(item, SidebarItem::Pane { .. }))
            .expect("should have a pane row");
        tree.cursor = pane_idx;

        let chrome = test_chrome();
        set_ws_collapsed(&mut tree, &chrome, 0, true);

        assert!(matches!(
            tree.current_item(),
            Some(SidebarItem::Workspace { ws_idx }) if *ws_idx == 0
        ));
    }

    #[test]
    fn test_collapse_column_moves_cursor_to_column_row() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        let pane_idx = tree
            .flat_items
            .iter()
            .position(|item| matches!(item, SidebarItem::Pane { .. }))
            .expect("should have a pane row");
        let (ws_idx, col_idx) = tree
            .flat_items
            .iter()
            .find_map(|item| match item {
                SidebarItem::Column { ws_idx, col_idx } => Some((*ws_idx, *col_idx)),
                _ => None,
            })
            .expect("should have a column row");
        tree.cursor = pane_idx;

        tree.collapse_column(ws_idx, col_idx);

        assert!(matches!(
            tree.current_item(),
            Some(SidebarItem::Column {
                ws_idx: item_ws,
                col_idx: item_col,
            }) if *item_ws == ws_idx && *item_col == col_idx
        ));
    }

    #[test]
    fn test_toggle_workspace_collapsed_by_index_updates_cursor() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        let pane_idx = tree
            .flat_items
            .iter()
            .position(|item| matches!(item, SidebarItem::Pane { .. }))
            .expect("should have a pane row");
        tree.cursor = pane_idx;
        let chrome = test_chrome();

        toggle_ws(&mut tree, &chrome, 0);
        assert!(tree.workspaces[0].collapsed);
        assert!(matches!(
            tree.current_item(),
            Some(SidebarItem::Workspace { ws_idx }) if *ws_idx == 0
        ));

        toggle_ws(&mut tree, &chrome, 0);
        assert!(!tree.workspaces[0].collapsed);
        assert!(matches!(
            tree.current_item(),
            Some(SidebarItem::Workspace { ws_idx }) if *ws_idx == 0
        ));
    }

    #[test]
    fn test_toggle_column_collapsed_by_index_updates_cursor() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        let pane_idx = tree
            .flat_items
            .iter()
            .position(|item| matches!(item, SidebarItem::Pane { .. }))
            .expect("should have a pane row");
        let (ws_idx, col_idx) = tree
            .flat_items
            .iter()
            .find_map(|item| match item {
                SidebarItem::Column { ws_idx, col_idx } => Some((*ws_idx, *col_idx)),
                _ => None,
            })
            .expect("should have a column row");
        tree.cursor = pane_idx;

        tree.toggle_column_collapsed(ws_idx, col_idx);
        assert!(tree.workspaces[ws_idx].columns[col_idx].collapsed);
        assert!(matches!(
            tree.current_item(),
            Some(SidebarItem::Column {
                ws_idx: item_ws,
                col_idx: item_col,
            }) if *item_ws == ws_idx && *item_col == col_idx
        ));

        tree.toggle_column_collapsed(ws_idx, col_idx);
        assert!(!tree.workspaces[ws_idx].columns[col_idx].collapsed);
        assert!(matches!(
            tree.current_item(),
            Some(SidebarItem::Column {
                ws_idx: item_ws,
                col_idx: item_col,
            }) if *item_ws == ws_idx && *item_col == col_idx
        ));
    }

    #[test]
    fn test_sidebar_hit_test_expanded() {
        let (session, _ids) = make_test_session();
        let tree = SidebarTree::new();
        // Rebuild into a fresh tree (cursor at 0)
        let mut tree = tree;
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // sidebar_top=32, 4px padding, then [+w] button row (24px), then first flat item.
        // First flat item starts at y = 32 + 4 + 24 = 60. Click middle of that row.
        let fi = sidebar_hit_test(&tree, 32.0, 400.0, 200.0, 60.0 + ITEM_HEIGHT / 2.0);
        assert_eq!(fi, Some(0), "click on first line should hit flat item 0");

        // Click above sidebar should miss.
        assert_eq!(sidebar_hit_test(&tree, 32.0, 400.0, 200.0, 10.0), None);

        // Click in the [+w] button row area should miss (returns None).
        let btn_row =
            sidebar_hit_test(&tree, 32.0, 400.0, 200.0, 32.0 + 4.0 + BTN_ROW_HEIGHT / 2.0);
        assert_eq!(btn_row, None, "click on [+w] button row should miss items");
    }

    #[test]
    fn test_collapse_persists_across_rebuild() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Collapse workspace 0.
        tree.cursor = 0;
        let chrome = test_chrome();
        tree.toggle_expand(&chrome.workspaces);
        assert!(
            tree.workspaces[0].collapsed,
            "WS 0 should be collapsed after toggle"
        );
        let collapsed_count = tree.flat_items.len();

        // Rebuild from session — sync defaults to expanded; the app re-applies the
        // canonical collapse set from chrome_state (as focus.rs does after sync).
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);
        let set = chrome.workspaces.with_collapsed_ws(|s| s.clone());
        tree.apply_ws_collapsed(&set, None);
        assert!(
            tree.workspaces[0].collapsed,
            "WS 0 collapse should persist across rebuild (via chrome_state)"
        );
        assert_eq!(
            tree.flat_items.len(),
            collapsed_count,
            "flat item count should remain reduced after rebuild"
        );
    }

    #[test]
    fn test_sidebar_hit_test_collapsed() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.sync_from_session(&session, None, Some(PaneId(1)), &[]);

        // Collapsed mode (width < 80). Click on second visible line.
        // Rows: 4px pad, [+w] row (24px), then visible lines.
        // Visible line 0 = WS (flat idx 0, column idx 1 skipped)
        // Visible line 1 = first Pane (flat idx 2)
        // First pane starts at y = 32 + 4 + 24 + 24 = 84.
        let first_pane_y = 32.0 + 4.0 + BTN_ROW_HEIGHT + ITEM_HEIGHT + ITEM_HEIGHT / 2.0;
        let fi = sidebar_hit_test(&tree, 32.0, 400.0, 40.0, first_pane_y);
        // Second visible line should be the first Pane (skipping the Column).
        assert_eq!(
            fi,
            Some(2),
            "second visible line in collapsed mode should be first Pane (flat idx 2)"
        );
    }
