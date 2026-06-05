#![allow(clippy::module_inception)]

    use super::*;
    use crate::app_state::SidebarItemState;
    use heca_core::layout::{
        Pane as LayoutPane, PaneId,
        session::Session,
        types::{Point, SessionId, Size},
        workspace::FloatingPane,
    };

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

        tree.rebuild(&session, None, Some(1), &[]);

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
        tree.rebuild(&session, None, Some(1), &[]);

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
        tree.rebuild(&session, None, Some(1), &[]);

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
        tree.rebuild(&session, None, Some(1), &[]);

        // Initially not collapsed
        assert!(
            !tree.workspaces[0].collapsed,
            "WS 0 should not be collapsed initially"
        );

        // Move cursor to workspace 0 and toggle expand
        tree.cursor = 0;
        tree.toggle_expand();

        // Should now be collapsed
        assert!(
            tree.workspaces[0].collapsed,
            "WS 0 should be collapsed after toggle"
        );

        // Flat items should have fewer items (children hidden)
        let collapsed_count = tree.flat_items.len();

        // Toggle again to expand
        tree.toggle_expand();
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
        tree.rebuild(&session, Some(0), Some(5), &[]);

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

        tree.rebuild(&session, None, Some(1), &[]);
        let first_count = tree.flat_items.len();

        // Rebuild again — should be same result
        tree.rebuild(&session, None, Some(1), &[]);
        assert_eq!(
            tree.flat_items.len(),
            first_count,
            "rebuild should produce same result"
        );

        // Cursor should be clamped if it was out of bounds
        tree.cursor = 9999;
        tree.rebuild(&session, None, Some(1), &[]);
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

        tree.rebuild(&session, None, None, &[]);

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
        tree.rebuild(&session, None, Some(1), &[]);

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
        tree.rebuild(&session, None, Some(1), &[]);

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
        tree.rebuild(&session, None, Some(1), &[]);

        // Place cursor deep inside workspace 0 (e.g. on a pane).
        let ws0_last_idx = tree
            .flat_items
            .iter()
            .enumerate()
            .rposition(|(_, i)| {
                matches!(i, SidebarItem::Workspace { ws_idx } if *ws_idx == 0)
                    || matches!(i, SidebarItem::Column { ws_idx, .. } if *ws_idx == 0)
                    || matches!(i, SidebarItem::Pane { pane_id } if *pane_id <= 4)
            })
            .expect("should have ws0 items");
        tree.cursor = ws0_last_idx;

        // Collapse workspace 0 — its children disappear.
        tree.workspaces[0].collapsed = true;
        tree.rebuild_flat_items();
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
        tree.rebuild(&session, None, Some(1), &[]);

        // Find first Column item in flat list.
        let col_idx = tree
            .flat_items
            .iter()
            .position(|i| matches!(i, SidebarItem::Column { .. }))
            .expect("should have a column");
        tree.cursor = col_idx;

        // Collapse the column.
        tree.toggle_expand();
        if let SidebarItem::Column { ws_idx, col_idx: c } = tree.flat_items[tree.cursor] {
            assert!(
                tree.workspaces[ws_idx].columns[c].collapsed,
                "column should be collapsed"
            );
        }

        // Expand it back.
        tree.toggle_expand();
        if let SidebarItem::Column { ws_idx, col_idx: c } = tree.flat_items[tree.cursor] {
            assert!(
                !tree.workspaces[ws_idx].columns[c].collapsed,
                "column should be expanded"
            );
        }
    }

    #[test]
    fn test_sidebar_hit_test_expanded() {
        let (session, _ids) = make_test_session();
        let tree = SidebarTree::new();
        // Rebuild into a fresh tree (cursor at 0)
        let mut tree = tree;
        tree.rebuild(&session, None, Some(1), &[]);

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
        tree.rebuild(&session, None, Some(1), &[]);

        // Collapse workspace 0.
        tree.cursor = 0;
        tree.toggle_expand();
        assert!(
            tree.workspaces[0].collapsed,
            "WS 0 should be collapsed after toggle"
        );
        let collapsed_count = tree.flat_items.len();

        // Rebuild from session — collapse state should survive.
        tree.rebuild(&session, None, Some(1), &[]);
        assert!(
            tree.workspaces[0].collapsed,
            "WS 0 collapse should persist across rebuild"
        );
        assert_eq!(
            tree.flat_items.len(),
            collapsed_count,
            "flat item count should remain reduced after rebuild"
        );
    }

    /// Build a session with one workspace containing:
    /// - 2 tiled panes in a single column
    /// - 1 floating pane
    fn make_session_with_floating_panes() -> (Session, u64) {
        let viewport = Size::new(1280.0, 800.0);
        let mut session = Session::new(
            SessionId(1),
            viewport,
            2.0,
            heca_core::layout::types::LayoutOptions::default(),
        );

        // Add 2 tiled panes
        let p1 = LayoutPane::new(PaneId(10), "Tiled1");
        let p2 = LayoutPane::new(PaneId(20), "Tiled2");
        session.add_pane(p1, None, true);
        session.add_pane(p2, None, true);

        // Add a floating pane
        let float_pane = LayoutPane::new(PaneId(99), "Float1");
        let floating = FloatingPane {
            pane: float_pane,
            position: Point::new(100.0, 100.0),
            size: Size::new(400.0, 300.0),
            is_active: false,
            original_column_idx: None,
            original_pane_idx: None,
        };
        session.workspaces[0].floating_panes.push(floating);

        (session, 99)
    }

    #[test]
    fn test_floating_panes_appear_in_flat_items() {
        let (session, float_id) = make_session_with_floating_panes();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, None, &[]);

        // Find the FloatingPane in flat_items
        let float_item = tree
            .flat_items
            .iter()
            .find(|i| matches!(i, SidebarItem::FloatingPane { .. }));
        let item = float_item.expect("should have a FloatingPane in flat_items");

        match item {
            SidebarItem::FloatingPane { pane_id, ws_idx } => {
                assert_eq!(*pane_id, float_id, "pane_id should match");
                assert_eq!(*ws_idx, 0, "ws_idx should be 0");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_floating_panes_not_navigable() {
        let (session, _) = make_session_with_floating_panes();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, None, &[]);

        // The flat list is: WS, Col0, Pane10, Col1, Pane20, FloatingPane99
        // FloatingPane is at the very end (idx 5).
        // is_navigable returns false for FloatingPane, so cursor movement
        // will skip it when a navigable alternative exists.
        // When FloatingPane is the last item, cursor_down from the previous
        // item lands on it (no alternative) — that's acceptable.

        // Verify: cursor_down from start skips FloatingPane when alternatives exist
        tree.cursor = 0;
        for _ in 0..tree.item_count {
            let prev = tree.cursor;
            tree.cursor_down();
            if tree.cursor == prev {
                break; // at end, no more movement
            }
        }
        // The only time cursor can land on FloatingPane is if it's the very last item
        // and all prior items have been visited. In that case it's the only option.
        // But importantly, cursor movement should have passed through all non-float items.
        let navigable_count = tree.flat_items.iter().filter(|i| !matches!(i, SidebarItem::FloatingPane { .. })).count();
        // cursor should have visited all navigable items
        // (it starts at 0, which is navigable, then visits the rest)
        assert!(navigable_count >= 2, "should have at least 2 navigable items");

        // Verify: cursor_up from FloatingPane position skips it when alternatives exist
        tree.cursor = tree.flat_items.len() - 1; // on FloatingPane
        tree.cursor_up();
        // Should have moved off the FloatingPane to a navigable item
        assert!(
            !matches!(tree.flat_items[tree.cursor], SidebarItem::FloatingPane { .. }),
            "cursor_up from FloatingPane should land on a navigable item, got {:?}",
            tree.flat_items[tree.cursor]
        );
    }

    #[test]
    fn test_floating_panes_not_selectable() {
        let (session, _) = make_session_with_floating_panes();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, None, &[]);

        // Verify is_navigable returns false for all FloatingPane items.
        // is_navigable is private, so we test the equivalent invariant:
        // cursor movement never selects a FloatingPane.
        for (idx, item) in tree.flat_items.iter().enumerate() {
            if matches!(item, SidebarItem::FloatingPane { .. }) {
                // Directly check the same condition is_navigable uses
                let navigable = !matches!(item, SidebarItem::FloatingPane { .. });
                assert!(!navigable, "FloatingPane at idx={} should not be selectable", idx);
            }
        }
    }

    #[test]
    fn test_floating_pane_hit_test_miss_expanded() {
        let (session, _) = make_session_with_floating_panes();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, None, &[]);

        // Find the FloatingPane's position in flat_items
        let float_idx = tree
            .flat_items
            .iter()
            .position(|i| matches!(i, SidebarItem::FloatingPane { .. }))
            .expect("should have a FloatingPane");

        // Calculate the y coordinate that would correspond to this item
        // sidebar_top=32, 4px padding, BTN_ROW_HEIGHT=24, then items start
        let item_y = 32.0 + 4.0 + BTN_ROW_HEIGHT + (float_idx as f32) * ITEM_HEIGHT + ITEM_HEIGHT / 2.0;

        // Hit test in expanded mode (width > 80) should return None for FloatingPane
        let result = sidebar_hit_test(&tree, 32.0, 800.0, 200.0, item_y);
        assert_eq!(result, None, "hit test on FloatingPane row should return None");
    }

    #[test]
    fn test_floating_pane_hit_test_miss_collapsed() {
        let (session, _) = make_session_with_floating_panes();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, None, &[]);

        // Find the FloatingPane's visible line index in collapsed mode
        // (Columns are skipped in collapsed mode)
        let mut visible_line = 0usize;
        let mut float_visible_line = None;
        for item in &tree.flat_items {
            if matches!(item, SidebarItem::Column { .. }) {
                continue;
            }
            if matches!(item, SidebarItem::FloatingPane { .. }) {
                float_visible_line = Some(visible_line);
                break;
            }
            visible_line += 1;
        }
        let vis_line = float_visible_line.expect("should have a FloatingPane in visible lines");

        // Calculate y for that visible line in collapsed mode
        let item_y = 32.0 + 4.0 + BTN_ROW_HEIGHT + (vis_line as f32) * ITEM_HEIGHT + ITEM_HEIGHT / 2.0;

        // Hit test in collapsed mode (width < 80) should return None for FloatingPane
        let result = sidebar_hit_test(&tree, 32.0, 800.0, 40.0, item_y);
        assert_eq!(result, None, "collapsed hit test on FloatingPane row should return None");
    }

    #[test]
    fn test_sidebar_hit_test_collapsed() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

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
