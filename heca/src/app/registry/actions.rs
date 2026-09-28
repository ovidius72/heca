//! **Action → handler.** Registers the handler for every built-in action. Owns nothing about keys.

use crate::actions::ActionRegistry;
use crate::handlers::*;
use crate::input::{self, SpawnKind, WmAction};
use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;

/// Build the action registry and register individual handlers for all actions.
pub fn build_registry() -> ActionRegistry {
    let mut registry = ActionRegistry::new();

    // ── Navigation ──
    registry.register(&WmAction::FocusLeft, handle_focus_left);
    registry.register(&WmAction::FocusRight, handle_focus_right);
    registry.register(&WmAction::FocusUp, handle_focus_up);
    registry.register(&WmAction::FocusDown, handle_focus_down);
    registry.register(&WmAction::NextPane, handle_next_pane);
    registry.register(&WmAction::PrevPane, handle_prev_pane);
    registry.register(&WmAction::WorkspaceNext, handle_workspace_next);
    registry.register(&WmAction::WorkspacePrev, handle_workspace_prev);
    registry.register(&WmAction::FocusToggleLocal, handle_focus_toggle_local);
    registry.register(&WmAction::FocusToggleGlobal, handle_focus_toggle_global);
    registry.register(
        &WmAction::FocusPane { pane_id: PaneId(0) },
        handle_focus_pane,
    );
    registry.register(
        &WmAction::FocusWorkspace { ws_idx: 0 },
        handle_focus_workspace,
    );

    // ── Layout ──
    registry.register(&WmAction::SplitHorizontal, handle_split_horizontal);
    registry.register(&WmAction::SplitVertical, handle_split_vertical);
    registry.register(&WmAction::ZoomColumn, handle_zoom_column);
    registry.register(
        &WmAction::ZoomColumnAtIndex {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_zoom_column_at_index,
    );
    registry.register(&WmAction::OpenContextMenu, handle_open_context_menu);
    registry.register(&WmAction::ScrollViewLeft, handle_scroll_view_left);
    registry.register(&WmAction::ScrollViewRight, handle_scroll_view_right);
    registry.register(&WmAction::ResizeIncrease, handle_resize_increase);
    registry.register(&WmAction::ResizeDecrease, handle_resize_decrease);
    registry.register(&WmAction::PaneHeightIncrease, handle_pane_height_increase);
    registry.register(&WmAction::PaneHeightDecrease, handle_pane_height_decrease);
    registry.register(&WmAction::SwapLeft, handle_swap_left);
    registry.register(&WmAction::SwapRight, handle_swap_right);
    registry.register(&WmAction::SwapUp, handle_swap_up);
    registry.register(&WmAction::SwapDown, handle_swap_down);
    registry.register(
        &WmAction::MovePaneLeft { pane_id: None },
        handle_move_pane_left,
    );
    registry.register(
        &WmAction::MovePaneRight { pane_id: None },
        handle_move_pane_right,
    );
    registry.register(&WmAction::MoveColumnUp, handle_move_column_up);
    registry.register(&WmAction::MoveColumnDown, handle_move_column_down);
    registry.register(
        &WmAction::Swap {
            a_id: PaneId(0),
            b_id: PaneId(0),
        },
        handle_swap_param,
    );
    registry.register(
        &WmAction::Move {
            pane_id: PaneId(0),
            target_col: 0,
        },
        handle_move_param,
    );
    registry.register(
        &WmAction::MovePaneToWorkspace {
            pane_id: PaneId(0),
            ws_idx: 0,
        },
        handle_move_pane_to_workspace,
    );
    registry.register(
        &WmAction::MovePaneToColumn {
            pane_id: PaneId(0),
            ws_idx: 0,
            col_idx: 0,
        },
        handle_move_pane_to_column,
    );
    registry.register(
        &WmAction::MoveColumnToWorkspace {
            col_idx: 0,
            ws_idx: 0,
            focus: true,
        },
        handle_move_column_to_workspace,
    );
    registry.register(
        &WmAction::MoveColumn {
            src_ws: 0,
            src_col: 0,
            dst_ws: 0,
            dst_idx: 0,
            focus: true,
        },
        handle_move_column,
    );
    registry.register(
        &WmAction::SwapColumns {
            a_ws: 0,
            a_col: 0,
            b_ws: 0,
            b_col: 0,
        },
        handle_swap_columns,
    );
    registry.register(
        &WmAction::Resize {
            target: input::ResizeTarget::Column,
            amount: 0.0,
            edge: input::ResizeEdge::Auto,
        },
        handle_resize,
    );
    registry.register(
        &WmAction::ResizeColumnBy {
            col_idx: 0,
            delta: 0.0,
        },
        handle_resize_column_by,
    );
    registry.register(
        &WmAction::ResizePaneHeightBy {
            col_idx: 0,
            pane_idx: 0,
            delta: 0.0,
        },
        handle_resize_pane_height_by,
    );
    registry.register(
        &WmAction::ResizeTo {
            target: input::ResizeTarget::Column,
            width: 0.0,
            height: 0.0,
        },
        handle_resize_to,
    );

    // ── Pane ──
    registry.register(&WmAction::Float, handle_float);
    registry.register(&WmAction::ClosePane, handle_close_pane);
    registry.register(&WmAction::PaneSelect, handle_pane_select);
    registry.register(&WmAction::FollowLink, handle_follow_link);
    registry.register(&WmAction::HintPick, handle_hint_pick);
    registry.register(&WmAction::SwapPane, handle_swap_pane);
    registry.register(&WmAction::SwapAndFocusPane, handle_swap_and_focus_pane);
    registry.register(
        &WmAction::MoveColumnToWorkspacePick,
        handle_move_column_to_workspace_pick,
    );
    registry.register(
        &WmAction::MovePaneToWorkspacePick,
        handle_move_pane_to_workspace_pick,
    );
    registry.register(
        &WmAction::MovePaneToColumnPick,
        handle_move_pane_to_column_pick,
    );
    registry.register(
        &WmAction::MovePaneToNewColumn,
        handle_move_pane_to_new_column,
    );
    registry.register(&WmAction::PaneTake, handle_pane_take);
    registry.register(&WmAction::PaneTakeAndFocus, handle_pane_take_and_focus);
    registry.register(
        &WmAction::TakePane {
            pane_id: PaneId(0),
            focus_after: false,
        },
        handle_take_pane,
    );
    // Chrome container placement (plugin-02, §2.9).
    registry.register(
        &WmAction::MoveContainerToRegion {
            container_id: String::new(),
            region: crate::chrome::RegionId::LeftSidebar,
        },
        handle_move_container_to_region,
    );
    registry.register(
        &WmAction::ReorderContainerBefore {
            container_id: String::new(),
            before_id: None,
        },
        handle_reorder_container_before,
    );
    registry.register(
        &WmAction::ReorderContainerAfter {
            container_id: String::new(),
            after_id: String::new(),
        },
        handle_reorder_container_after,
    );
    registry.register(
        &WmAction::SetRegionVisible {
            region: crate::chrome::RegionId::LeftSidebar,
            visible: false,
        },
        handle_set_region_visible,
    );
    // Chrome region show/hide mounted-gate (sidebar-fu-6) — 12 unit actions, one handler.
    for action in [
        &WmAction::ShowLeftSidebar,
        &WmAction::HideLeftSidebar,
        &WmAction::ToggleLeftSidebar,
        &WmAction::ShowRightSidebar,
        &WmAction::HideRightSidebar,
        &WmAction::ToggleRightSidebar,
        &WmAction::ShowTopBar,
        &WmAction::HideTopBar,
        &WmAction::ToggleTopBar,
        &WmAction::ShowBottomBar,
        &WmAction::HideBottomBar,
        &WmAction::ToggleBottomBar,
    ] {
        registry.register(action, handle_set_chrome_region_shown);
    }
    registry.register(&WmAction::RenamePane, handle_rename_pane);
    registry.register(&WmAction::RenameColumn, handle_rename_column);
    registry.register(
        &WmAction::RenameColumnByIdx {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_rename_column_by_idx,
    );
    registry.register(
        &WmAction::FloatAt {
            pane_id: PaneId(0),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        handle_float_at,
    );
    registry.register(
        &WmAction::ClosePaneById { pane_id: PaneId(0) },
        handle_close_pane_by_id,
    );
    registry.register(
        &WmAction::RenameTarget {
            pane_id: PaneId(0),
            name: String::new(),
        },
        handle_rename_target,
    );
    registry.register(
        &WmAction::RenamePaneById { pane_id: PaneId(0) },
        handle_rename_pane_by_id,
    );
    registry.register(&WmAction::ResetPaneName, handle_reset_pane_name);
    registry.register(
        &WmAction::ResetPaneNameById { pane_id: PaneId(0) },
        handle_reset_pane_name_by_id,
    );

    // ── Workspace ──
    registry.register(&WmAction::CreateWorkspace, handle_create_workspace);
    registry.register(&WmAction::RenameWorkspace, handle_rename_workspace);
    registry.register(
        &WmAction::RenameWorkspaceByIdx { ws_idx: 0 },
        handle_rename_workspace_by_idx,
    );
    registry.register(&WmAction::ResetWorkspaceName, handle_reset_workspace_name);
    registry.register(
        &WmAction::ResetWorkspaceNameByIdx { ws_idx: 0 },
        handle_reset_workspace_name_by_idx,
    );

    // ── Sidebar / Chrome ──
    registry.register(&WmAction::SidebarLeft, handle_sidebar_left);
    registry.register(&WmAction::SidebarRight, handle_sidebar_right);
    registry.register(&WmAction::FocusDock { dock: None }, handle_focus_dock);
    registry.register(
        &WmAction::CursorTo {
            mount: String::new(),
            key: String::new(),
        },
        handle_cursor_to,
    );
    registry.register(&WmAction::ToggleDock { dock: None }, handle_toggle_dock);
    registry.register(
        &WmAction::ClearSearchHistory { scope: None },
        crate::handlers::handle_clear_search_history,
    );
    registry.register(
        &WmAction::ClearSearchRanking { scope: None },
        crate::handlers::handle_clear_search_ranking,
    );
    registry.register(&WmAction::UnfocusDock, handle_unfocus_dock);
    registry.register(
        &WmAction::CollapseCurrentWorkspace,
        handle_collapse_current_workspace,
    );
    registry.register(
        &WmAction::ExpandCurrentWorkspace,
        handle_expand_current_workspace,
    );
    registry.register(
        &WmAction::ToggleCurrentWorkspaceCollapsed,
        handle_toggle_current_workspace_collapsed,
    );
    registry.register(
        &WmAction::CollapseCurrentColumn,
        handle_collapse_current_column,
    );
    registry.register(&WmAction::ExpandCurrentColumn, handle_expand_current_column);
    registry.register(
        &WmAction::ToggleCurrentColumnCollapsed,
        handle_toggle_current_column_collapsed,
    );

    // ── System ──
    registry.register(
        &WmAction::CommandPalette {
            mode: None,
            query: None,
        },
        handle_command_palette,
    );
    registry.register(
        &WmAction::CloseOverlay { overlay: None },
        crate::handlers::handle_close_overlay,
    );
    for act in [
        WmAction::ShowLayer {
            name: None,
            dock: None,
        },
        WmAction::HideLayer {
            name: None,
            dock: None,
        },
        WmAction::ToggleLayer {
            name: None,
            dock: None,
        },
    ] {
        registry.register(&act, crate::handlers::handle_layer_visibility);
    }
    registry.register(
        &WmAction::SpawnCommand {
            command: String::new(),
            kind: SpawnKind::Terminal,
            float: false,
            close_policy: PaneClosePolicy::default(),
        },
        handle_spawn_command,
    );
    registry.register(&WmAction::ReloadConfig, handle_reload_config);
    registry.register(
        &WmAction::NotificationDismissOne { notification_id: 0 },
        handle_notification_dismiss_one,
    );
    registry.register(
        &WmAction::NotificationDismissAll,
        handle_notification_dismiss_all,
    );
    registry.register(
        &WmAction::NotificationDismissLast,
        handle_notification_dismiss_last,
    );
    registry.register(&WmAction::NotificationPick, handle_notification_pick);
    registry.register(
        &WmAction::NotificationActionRelay {
            notification_id: 0,
            key: String::new(),
        },
        handle_notification_action_relay,
    );
    registry.register(&WmAction::OpenLink { url: String::new() }, handle_open_link);

    // ── Font zoom (terminal) ──
    registry.register(
        &WmAction::AppFontZoom {
            step: input::FontZoomStep::In,
        },
        handle_app_font_zoom,
    );
    registry.register(
        &WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: input::FontZoomStep::In,
        },
        handle_pane_terminal_font_zoom,
    );

    // ── Sidebar-specific (parameterized) ──
    registry.register(
        &WmAction::AddPaneToColumn {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_add_pane_to_column,
    );
    registry.register(
        &WmAction::AddColumnToWorkspace { ws_idx: 0 },
        handle_add_column_to_workspace,
    );

    // ── Destructive ──
    registry.register(
        &WmAction::DeleteColumn {
            ws_idx: 0,
            col_idx: 0,
        },
        handle_delete_column,
    );
    registry.register(
        &WmAction::DeleteWorkspace { ws_idx: 0 },
        handle_delete_workspace,
    );
    registry.register(&WmAction::DeleteCurrentColumn, handle_delete_current_column);

    // ── Take ──
    // (registered above with PaneTake/PaneTakeAndFocus)

    // ── Mode ──
    registry.register(
        &WmAction::EnterMode {
            name: String::new(),
        },
        handle_enter_mode,
    );

    // ── Scrollback (host terminal viewport) ──
    registry.register(&WmAction::ScrollbackPageUp, handle_scrollback_page_up);
    registry.register(&WmAction::ScrollbackPageDown, handle_scrollback_page_down);
    registry.register(
        &WmAction::ScrollbackLineUp { amount: 3 },
        handle_scrollback_line_up,
    );
    registry.register(
        &WmAction::ScrollbackLineDown { amount: 3 },
        handle_scrollback_line_down,
    );
    registry.register(&WmAction::ScrollbackToTop, handle_scrollback_to_top);
    registry.register(&WmAction::ScrollbackToBottom, handle_scrollback_to_bottom);
    registry.register(&WmAction::ExitScrollback, handle_exit_scrollback);

    // ── Direct scroll (no selection mode / caret) ──
    registry.register(&WmAction::ScrollLineUp, handle_scroll_line_up);
    registry.register(&WmAction::ScrollLineDown, handle_scroll_line_down);
    registry.register(&WmAction::ScrollPageUp, handle_scroll_page_up);
    registry.register(&WmAction::ScrollPageDown, handle_scroll_page_down);
    registry.register(&WmAction::ScrollToTop, handle_scroll_to_top);
    registry.register(&WmAction::ScrollToBottom, handle_scroll_to_bottom);
    registry.register(&WmAction::ScrollPageLeft, handle_scroll_page_left);
    registry.register(&WmAction::ScrollPageRight, handle_scroll_page_right);
    registry.register(&WmAction::ScrollToLeftEdge, handle_scroll_to_left_edge);
    registry.register(&WmAction::ScrollToRightEdge, handle_scroll_to_right_edge);
    registry.register(
        &WmAction::ScrollToOffset { rows: 0 },
        handle_scroll_to_offset,
    );

    // ── Selection (host capability) ──
    registry.register(&WmAction::EnterSelectionMode, handle_enter_selection_mode);
    registry.register(&WmAction::SelectionLeft, handle_selection_left);
    registry.register(&WmAction::SelectionRight, handle_selection_right);
    registry.register(&WmAction::SelectionUp, handle_selection_up);
    registry.register(&WmAction::SelectionDown, handle_selection_down);
    registry.register(&WmAction::ClearSelection, handle_clear_selection);
    registry.register(&WmAction::CopySelection, handle_copy_selection);
    registry.register(&WmAction::PasteClipboard, handle_paste_clipboard);
    registry.register(&WmAction::BeginSelection, handle_begin_selection);
    registry.register(
        &WmAction::ToggleSelectionEndpoint,
        handle_toggle_selection_endpoint,
    );
    registry.register(&WmAction::OpenLinkAtCaret, handle_open_link_at_caret);
    registry.register(&WmAction::SearchScrollback, handle_search_scrollback);
    registry.register(&WmAction::SearchNextMatch, handle_search_next_match);
    registry.register(&WmAction::SearchPrevMatch, handle_search_prev_match);

    registry
}

#[cfg(test)]
mod tests;
