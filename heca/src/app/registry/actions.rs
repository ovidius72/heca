//! **Action → handler.** Registers the handler for every built-in action. Owns nothing about keys.

use crate::actions::ActionRegistry;
use crate::handlers::*;
use crate::input::WmActionKind;

/// Build the action registry and register individual handlers for all actions.
pub fn build_registry() -> ActionRegistry {
    let mut registry = ActionRegistry::new();

    // ── Navigation ──
    registry.register(WmActionKind::FocusLeft, handle_focus_left);
    registry.register(WmActionKind::FocusRight, handle_focus_right);
    registry.register(WmActionKind::FocusUp, handle_focus_up);
    registry.register(WmActionKind::FocusDown, handle_focus_down);
    registry.register(WmActionKind::NextPane, handle_next_pane);
    registry.register(WmActionKind::PrevPane, handle_prev_pane);
    registry.register(WmActionKind::WorkspaceNext, handle_workspace_next);
    registry.register(WmActionKind::WorkspacePrev, handle_workspace_prev);
    registry.register(WmActionKind::FocusToggleLocal, handle_focus_toggle_local);
    registry.register(WmActionKind::FocusToggleGlobal, handle_focus_toggle_global);
    registry.register(WmActionKind::FocusPane, handle_focus_pane);
    registry.register(WmActionKind::FocusWorkspace, handle_focus_workspace);

    // ── Layout ──
    registry.register(WmActionKind::SplitHorizontal, handle_split_horizontal);
    registry.register(WmActionKind::SplitVertical, handle_split_vertical);
    registry.register(WmActionKind::ZoomColumn, handle_zoom_column);
    registry.register(WmActionKind::ZoomColumnAtIndex, handle_zoom_column_at_index);
    registry.register(WmActionKind::OpenContextMenu, handle_open_context_menu);
    registry.register(WmActionKind::ScrollViewLeft, handle_scroll_view_left);
    registry.register(WmActionKind::ScrollViewRight, handle_scroll_view_right);
    registry.register(WmActionKind::ResizeIncrease, handle_resize_increase);
    registry.register(WmActionKind::ResizeDecrease, handle_resize_decrease);
    registry.register(
        WmActionKind::PaneHeightIncrease,
        handle_pane_height_increase,
    );
    registry.register(
        WmActionKind::PaneHeightDecrease,
        handle_pane_height_decrease,
    );
    registry.register(WmActionKind::SwapLeft, handle_swap_left);
    registry.register(WmActionKind::SwapRight, handle_swap_right);
    registry.register(WmActionKind::SwapUp, handle_swap_up);
    registry.register(WmActionKind::SwapDown, handle_swap_down);
    registry.register(WmActionKind::MovePaneLeft, handle_move_pane_left);
    registry.register(WmActionKind::MovePaneRight, handle_move_pane_right);
    registry.register(WmActionKind::MoveColumnUp, handle_move_column_up);
    registry.register(WmActionKind::MoveColumnDown, handle_move_column_down);
    registry.register(WmActionKind::Swap, handle_swap_param);
    registry.register(WmActionKind::Move, handle_move_param);
    registry.register(
        WmActionKind::MovePaneToWorkspace,
        handle_move_pane_to_workspace,
    );
    registry.register(WmActionKind::MovePaneToColumn, handle_move_pane_to_column);
    registry.register(WmActionKind::PlacePane, handle_place_pane);
    registry.register(
        WmActionKind::MoveColumnToWorkspace,
        handle_move_column_to_workspace,
    );
    registry.register(WmActionKind::MoveColumn, handle_move_column);
    registry.register(WmActionKind::SwapColumns, handle_swap_columns);
    registry.register(WmActionKind::Resize, handle_resize);
    registry.register(WmActionKind::ResizeColumnBy, handle_resize_column_by);
    registry.register(
        WmActionKind::ResizePaneHeightBy,
        handle_resize_pane_height_by,
    );
    registry.register(WmActionKind::ResizeTo, handle_resize_to);

    // ── Pane ──
    registry.register(WmActionKind::Float, handle_float);
    registry.register(WmActionKind::ClosePane, handle_close_pane);
    registry.register(WmActionKind::PaneSelect, handle_pane_select);
    registry.register(WmActionKind::FollowLink, handle_follow_link);
    registry.register(WmActionKind::HintPick, handle_hint_pick);
    registry.register(WmActionKind::SwapPane, handle_swap_pane);
    registry.register(WmActionKind::SwapAndFocusPane, handle_swap_and_focus_pane);
    registry.register(
        WmActionKind::MoveColumnToWorkspacePick,
        handle_move_column_to_workspace_pick,
    );
    registry.register(
        WmActionKind::MovePaneToWorkspacePick,
        handle_move_pane_to_workspace_pick,
    );
    registry.register(
        WmActionKind::MovePaneToColumnPick,
        handle_move_pane_to_column_pick,
    );
    registry.register(
        WmActionKind::MovePaneToNewColumn,
        handle_move_pane_to_new_column,
    );
    registry.register(WmActionKind::PaneTake, handle_pane_take);
    registry.register(WmActionKind::PaneTakeAndFocus, handle_pane_take_and_focus);
    registry.register(WmActionKind::TakePane, handle_take_pane);
    // Chrome container placement (plugin-02, §2.9).
    registry.register(
        WmActionKind::MoveContainerToRegion,
        handle_move_container_to_region,
    );
    registry.register(
        WmActionKind::ReorderContainerBefore,
        handle_reorder_container_before,
    );
    registry.register(
        WmActionKind::ReorderContainerAfter,
        handle_reorder_container_after,
    );
    registry.register(WmActionKind::SetRegionVisible, handle_set_region_visible);
    registry.register(WmActionKind::RenamePane, handle_rename_pane);
    registry.register(WmActionKind::RenameColumn, handle_rename_column);
    registry.register(WmActionKind::RenameColumnByIdx, handle_rename_column_by_idx);
    registry.register(WmActionKind::FloatAt, handle_float_at);
    registry.register(WmActionKind::ClosePaneById, handle_close_pane_by_id);
    registry.register(WmActionKind::RenameTarget, handle_rename_target);
    registry.register(WmActionKind::RenamePaneById, handle_rename_pane_by_id);
    registry.register(WmActionKind::ResetPaneName, handle_reset_pane_name);
    registry.register(
        WmActionKind::ResetPaneNameById,
        handle_reset_pane_name_by_id,
    );

    // ── Workspace ──
    registry.register(WmActionKind::CreateWorkspace, handle_create_workspace);
    registry.register(WmActionKind::RenameWorkspace, handle_rename_workspace);
    registry.register(
        WmActionKind::RenameWorkspaceByIdx,
        handle_rename_workspace_by_idx,
    );
    registry.register(
        WmActionKind::ResetWorkspaceName,
        handle_reset_workspace_name,
    );
    registry.register(
        WmActionKind::ResetWorkspaceNameByIdx,
        handle_reset_workspace_name_by_idx,
    );

    // ── Sidebar / Chrome ──
    registry.register(WmActionKind::SidebarLeft, handle_sidebar_left);
    registry.register(WmActionKind::SidebarRight, handle_sidebar_right);
    registry.register(WmActionKind::FocusDock, handle_focus_dock);
    registry.register(WmActionKind::CursorTo, handle_cursor_to);
    registry.register(WmActionKind::ToggleDock, handle_toggle_dock);
    registry.register(
        WmActionKind::ClearSearchHistory,
        crate::handlers::handle_clear_search_history,
    );
    registry.register(
        WmActionKind::ClearSearchRanking,
        crate::handlers::handle_clear_search_ranking,
    );
    registry.register(WmActionKind::UnfocusDock, handle_unfocus_dock);
    registry.register(
        WmActionKind::CollapseCurrentWorkspace,
        handle_collapse_current_workspace,
    );
    registry.register(
        WmActionKind::ExpandCurrentWorkspace,
        handle_expand_current_workspace,
    );
    registry.register(
        WmActionKind::ToggleCurrentWorkspaceCollapsed,
        handle_toggle_current_workspace_collapsed,
    );
    registry.register(
        WmActionKind::CollapseCurrentColumn,
        handle_collapse_current_column,
    );
    registry.register(
        WmActionKind::ExpandCurrentColumn,
        handle_expand_current_column,
    );
    registry.register(
        WmActionKind::ToggleCurrentColumnCollapsed,
        handle_toggle_current_column_collapsed,
    );

    // ── System ──
    registry.register(WmActionKind::CommandPalette, handle_command_palette);
    for kind in [
        WmActionKind::ShowLayer,
        WmActionKind::HideLayer,
        WmActionKind::ToggleLayer,
    ] {
        registry.register(kind, crate::handlers::handle_layer_visibility);
    }
    registry.register(WmActionKind::SpawnCommand, handle_spawn_command);
    registry.register(WmActionKind::ReloadConfig, handle_reload_config);
    registry.register(
        WmActionKind::NotificationDismissOne,
        handle_notification_dismiss_one,
    );
    registry.register(
        WmActionKind::NotificationDismissAll,
        handle_notification_dismiss_all,
    );
    registry.register(
        WmActionKind::NotificationDismissLast,
        handle_notification_dismiss_last,
    );
    registry.register(WmActionKind::NotificationPick, handle_notification_pick);
    registry.register(
        WmActionKind::NotificationActionRelay,
        handle_notification_action_relay,
    );
    registry.register(WmActionKind::OpenLink, handle_open_link);

    // ── Font zoom (terminal) ──
    registry.register(WmActionKind::AppFontZoom, handle_app_font_zoom);
    registry.register(
        WmActionKind::PaneTerminalFontZoom,
        handle_pane_terminal_font_zoom,
    );

    // ── Terminal (run a line, end it) ──
    registry.register(WmActionKind::TerminalRun, handle_terminal_run);
    registry.register(WmActionKind::TerminalKill, handle_terminal_kill);

    // ── Sidebar-specific (parameterized) ──
    registry.register(WmActionKind::AddPaneToColumn, handle_add_pane_to_column);
    registry.register(
        WmActionKind::AddColumnToWorkspace,
        handle_add_column_to_workspace,
    );

    // ── Destructive ──
    registry.register(WmActionKind::DeleteColumn, handle_delete_column);
    registry.register(WmActionKind::DeleteWorkspace, handle_delete_workspace);
    registry.register(
        WmActionKind::DeleteCurrentColumn,
        handle_delete_current_column,
    );

    // ── Take ──
    // (registered above with PaneTake/PaneTakeAndFocus)

    // ── Mode ──

    // ── Scrollback (host terminal viewport) ──
    registry.register(WmActionKind::ScrollbackPageUp, handle_scrollback_page_up);
    registry.register(
        WmActionKind::ScrollbackPageDown,
        handle_scrollback_page_down,
    );
    registry.register(WmActionKind::ScrollbackLineUp, handle_scrollback_line_up);
    registry.register(
        WmActionKind::ScrollbackLineDown,
        handle_scrollback_line_down,
    );
    registry.register(WmActionKind::ScrollbackToTop, handle_scrollback_to_top);
    registry.register(
        WmActionKind::ScrollbackToBottom,
        handle_scrollback_to_bottom,
    );
    registry.register(WmActionKind::ExitScrollback, handle_exit_scrollback);

    // ── Direct scroll (no selection mode / caret) ──
    registry.register(WmActionKind::ScrollLineUp, handle_scroll_line_up);
    registry.register(WmActionKind::ScrollLineDown, handle_scroll_line_down);
    registry.register(WmActionKind::ScrollPageUp, handle_scroll_page_up);
    registry.register(WmActionKind::ScrollPageDown, handle_scroll_page_down);
    registry.register(WmActionKind::ScrollToTop, handle_scroll_to_top);
    registry.register(WmActionKind::ScrollToBottom, handle_scroll_to_bottom);
    registry.register(WmActionKind::ScrollPageLeft, handle_scroll_page_left);
    registry.register(WmActionKind::ScrollPageRight, handle_scroll_page_right);
    registry.register(WmActionKind::ScrollToLeftEdge, handle_scroll_to_left_edge);
    registry.register(WmActionKind::ScrollToRightEdge, handle_scroll_to_right_edge);
    registry.register(WmActionKind::ScrollToOffset, handle_scroll_to_offset);

    // ── Selection (host capability) ──
    registry.register(
        WmActionKind::EnterSelectionMode,
        handle_enter_selection_mode,
    );
    registry.register(WmActionKind::SelectionLeft, handle_selection_left);
    registry.register(WmActionKind::SelectionRight, handle_selection_right);
    registry.register(WmActionKind::SelectionUp, handle_selection_up);
    registry.register(WmActionKind::SelectionDown, handle_selection_down);
    registry.register(WmActionKind::ClearSelection, handle_clear_selection);
    registry.register(WmActionKind::CopySelection, handle_copy_selection);
    registry.register(WmActionKind::PasteClipboard, handle_paste_clipboard);
    registry.register(WmActionKind::BeginSelection, handle_begin_selection);
    registry.register(
        WmActionKind::ToggleSelectionEndpoint,
        handle_toggle_selection_endpoint,
    );
    registry.register(WmActionKind::OpenLinkAtCaret, handle_open_link_at_caret);
    registry.register(WmActionKind::SearchScrollback, handle_search_scrollback);
    registry.register(WmActionKind::SearchNextMatch, handle_search_next_match);
    registry.register(WmActionKind::SearchPrevMatch, handle_search_prev_match);

    registry
}

#[cfg(test)]
mod tests;
