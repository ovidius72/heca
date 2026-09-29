//! An action from its name alone — only for the ones whose bare name says everything.

use super::{FontZoomStep, WmAction};

/**
Map an action name to a `WmAction` that needs **no arguments** to be meaningful.

A variant appears here only when the bare name says everything: a unit variant, or one whose
fields all have a real default (`move_pane_left`'s `pane_id: None` means "the focused pane";
`app_font_increase` carries its own step; `scrollback_line_up`'s one notch is the documented
default). An action that needs a target is built from its declared arguments through
[`build_action`] instead — see the note at the placeholder arms that used to live here.
*/
pub(super) fn action_from_name(name: &str) -> Option<WmAction> {
    match name {
        "focus_left" => Some(WmAction::FocusLeft),
        "focus_right" => Some(WmAction::FocusRight),
        "focus_up" => Some(WmAction::FocusUp),
        "focus_down" => Some(WmAction::FocusDown),
        "split_horizontal" => Some(WmAction::SplitHorizontal),
        "split_vertical" => Some(WmAction::SplitVertical),
        "zoom_column" => Some(WmAction::ZoomColumn),
        "open_context_menu" => Some(WmAction::OpenContextMenu),
        "scroll_view_left" => Some(WmAction::ScrollViewLeft),
        "scroll_view_right" => Some(WmAction::ScrollViewRight),
        "float" => Some(WmAction::Float),
        "close" => Some(WmAction::ClosePane),
        "delete_current_column" => Some(WmAction::DeleteCurrentColumn),
        "hint_pick" => Some(WmAction::HintPick),
        "resize_increase" => Some(WmAction::ResizeIncrease),
        "resize_decrease" => Some(WmAction::ResizeDecrease),
        "sidebar_left" => Some(WmAction::SidebarLeft),
        "sidebar_right" => Some(WmAction::SidebarRight),
        // Bare: no dock named ⇒ pick one by letter. `dock = "…"` goes through `build_action`.
        "focus_dock" => Some(WmAction::FocusDock { dock: None }),
        // The same, and back out again if it already has the keyboard — what `global_focus` binds.
        // Separate from `focus_dock` so a click, an RPC call and the palette cannot release a dock
        // by asking to focus it (F003/P082/T444).
        "toggle_dock" => Some(WmAction::ToggleDock { dock: None }),
        "unfocus_dock" => Some(WmAction::UnfocusDock),
        "collapse_current_workspace" => Some(WmAction::CollapseCurrentWorkspace),
        "expand_current_workspace" => Some(WmAction::ExpandCurrentWorkspace),
        "toggle_current_workspace_collapsed" => Some(WmAction::ToggleCurrentWorkspaceCollapsed),
        "collapse_current_column" => Some(WmAction::CollapseCurrentColumn),
        "expand_current_column" => Some(WmAction::ExpandCurrentColumn),
        "toggle_current_column_collapsed" => Some(WmAction::ToggleCurrentColumnCollapsed),
        "next_pane" => Some(WmAction::NextPane),
        "prev_pane" => Some(WmAction::PrevPane),
        "pane_select" => Some(WmAction::PaneSelect),
        "follow_link" => Some(WmAction::FollowLink),
        "swap_pane" => Some(WmAction::SwapPane),
        "move_column_to_workspace_pick" => Some(WmAction::MoveColumnToWorkspacePick),
        "move_pane_to_workspace_pick" => Some(WmAction::MovePaneToWorkspacePick),
        "move_pane_to_column_pick" => Some(WmAction::MovePaneToColumnPick),
        // The bare name says everything: it acts on the focused pane and needs no target.
        "move_pane_to_new_column" => Some(WmAction::MovePaneToNewColumn),
        "pane_take" => Some(WmAction::PaneTake),
        "pane_take_and_focus" => Some(WmAction::PaneTakeAndFocus),
        "swap_and_focus_pane" => Some(WmAction::SwapAndFocusPane),
        "swap_left" => Some(WmAction::SwapLeft),
        "swap_right" => Some(WmAction::SwapRight),
        "swap_up" => Some(WmAction::SwapUp),
        "swap_down" => Some(WmAction::SwapDown),
        "move_pane_left" => Some(WmAction::MovePaneLeft { pane_id: None }),
        "move_pane_right" => Some(WmAction::MovePaneRight { pane_id: None }),
        "move_column_up" => Some(WmAction::MoveColumnUp),
        "move_column_down" => Some(WmAction::MoveColumnDown),
        // NOTE: the actions that *need* a target — `move_pane_to_workspace`, `move_pane_to_column`,
        // `move_column_to_workspace`, `move_column`, `swap_columns`, `add_pane_to_column`,
        // `delete_column`, `delete_workspace` — are deliberately NOT here. They used to be, each
        // returning a variant with every index filled with 0, so binding a bare `delete_workspace`
        // to a key deleted **workspace 0** rather than doing nothing. They are built from their
        // declared arguments through `build_action`, and a call that omits one is now reported
        // instead of quietly becoming a call on index 0
        // (`every_action_that_needs_a_target_refuses_to_default_it` holds the line).
        "pane_height_increase" => Some(WmAction::PaneHeightIncrease),
        "pane_height_decrease" => Some(WmAction::PaneHeightDecrease),
        // Font zoom — global (app-wide terminal) branch, the `app-03` base.
        "app_font_increase" => Some(WmAction::AppFontZoom {
            step: FontZoomStep::In,
        }),
        "app_font_decrease" => Some(WmAction::AppFontZoom {
            step: FontZoomStep::Out,
        }),
        "app_font_reset" => Some(WmAction::AppFontZoom {
            step: FontZoomStep::Reset,
        }),
        // Font zoom — focused-pane branch (keyboard resolves `pane_id = None`).
        "pane_terminal_font_increase" => Some(WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::In,
        }),
        "pane_terminal_font_decrease" => Some(WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::Out,
        }),
        "pane_terminal_font_reset" => Some(WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::Reset,
        }),
        "workspace_next" => Some(WmAction::WorkspaceNext),
        "focus_toggle_local" => Some(WmAction::FocusToggleLocal),
        "focus_toggle_global" => Some(WmAction::FocusToggleGlobal),
        "workspace_prev" => Some(WmAction::WorkspacePrev),
        "create_workspace" => Some(WmAction::CreateWorkspace),
        "rename_workspace" => Some(WmAction::RenameWorkspace),
        "rename_pane" => Some(WmAction::RenamePane),
        "rename_column" => Some(WmAction::RenameColumn),
        "reset_pane_name" => Some(WmAction::ResetPaneName),
        "reset_workspace_name" => Some(WmAction::ResetWorkspaceName),
        "command_palette" => Some(WmAction::CommandPalette {
            mode: None,
            query: None,
        }),
        "close_overlay" => Some(WmAction::CloseOverlay { overlay: None }),
        "show_layer" => Some(WmAction::ShowLayer {
            name: None,
            dock: None,
        }),
        "hide_layer" => Some(WmAction::HideLayer {
            name: None,
            dock: None,
        }),
        "toggle_layer" => Some(WmAction::ToggleLayer {
            name: None,
            dock: None,
        }),
        "reload_config" => Some(WmAction::ReloadConfig),
        "notification_dismiss_all" => Some(WmAction::NotificationDismissAll),
        "notification_dismiss_last" => Some(WmAction::NotificationDismissLast),
        "notification_pick" => Some(WmAction::NotificationPick),
        "clear_search_history" => Some(WmAction::ClearSearchHistory { scope: None }),
        "clear_search_ranking" => Some(WmAction::ClearSearchRanking { scope: None }),
        // Scrollback
        "scrollback_page_up" => Some(WmAction::ScrollbackPageUp),
        "scrollback_page_down" => Some(WmAction::ScrollbackPageDown),
        "scrollback_to_top" => Some(WmAction::ScrollbackToTop),
        "scrollback_to_bottom" => Some(WmAction::ScrollbackToBottom),
        "exit_scrollback" => Some(WmAction::ExitScrollback),
        // Direct scroll (no selection mode / caret)
        "scroll_line_up" => Some(WmAction::ScrollLineUp),
        "scroll_line_down" => Some(WmAction::ScrollLineDown),
        "scroll_page_up" => Some(WmAction::ScrollPageUp),
        "scroll_page_down" => Some(WmAction::ScrollPageDown),
        "scroll_to_top" => Some(WmAction::ScrollToTop),
        "scroll_to_bottom" => Some(WmAction::ScrollToBottom),
        "scroll_page_left" => Some(WmAction::ScrollPageLeft),
        "scroll_page_right" => Some(WmAction::ScrollPageRight),
        "scroll_to_left_edge" => Some(WmAction::ScrollToLeftEdge),
        "scroll_to_right_edge" => Some(WmAction::ScrollToRightEdge),
        // `amount` is in notches; the handler multiplies by the user-configurable
        // `terminal_wheel_scroll_lines` before scrolling.  Default = 1 notch.
        "scrollback_line_up" => Some(WmAction::ScrollbackLineUp { amount: 1 }),
        "scrollback_line_down" => Some(WmAction::ScrollbackLineDown { amount: 1 }),

        "enter_selection_mode" => Some(WmAction::EnterSelectionMode),
        "selection_left" => Some(WmAction::SelectionLeft),
        "selection_right" => Some(WmAction::SelectionRight),
        "selection_up" => Some(WmAction::SelectionUp),
        "selection_down" => Some(WmAction::SelectionDown),
        "clear_selection" => Some(WmAction::ClearSelection),
        "copy_selection" => Some(WmAction::CopySelection),
        "paste_clipboard" => Some(WmAction::PasteClipboard),
        "begin_selection" => Some(WmAction::BeginSelection),
        "toggle_selection_endpoint" => Some(WmAction::ToggleSelectionEndpoint),
        "open_link_at_caret" => Some(WmAction::OpenLinkAtCaret),
        "search_scrollback" => Some(WmAction::SearchScrollback),
        "search_next_match" => Some(WmAction::SearchNextMatch),
        "search_prev_match" => Some(WmAction::SearchPrevMatch),
        _ => {
            // Dynamic: focus_workspace_1 → FocusWorkspace { ws_idx: 0 }
            if let Some(rest) = name.strip_prefix("focus_workspace_")
                && let Ok(n) = rest.parse::<usize>()
                && n >= 1
            {
                return Some(WmAction::FocusWorkspace { ws_idx: n - 1 });
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_from_name_known() {
        assert_eq!(action_from_name("focus_left"), Some(WmAction::FocusLeft));
        assert_eq!(action_from_name("focus_right"), Some(WmAction::FocusRight));
        assert_eq!(action_from_name("zoom_column"), Some(WmAction::ZoomColumn));
        assert_eq!(
            action_from_name("toggle_current_workspace_collapsed"),
            Some(WmAction::ToggleCurrentWorkspaceCollapsed)
        );
        assert_eq!(
            action_from_name("toggle_current_column_collapsed"),
            Some(WmAction::ToggleCurrentColumnCollapsed)
        );
        assert_eq!(
            action_from_name("rename_column"),
            Some(WmAction::RenameColumn)
        );
        assert_eq!(
            action_from_name("rename_column_by_idx"),
            None,
            "by-idx variants are menu/RPC-only and carry args, so they are not resolvable by bare name"
        );
        assert_eq!(
            action_from_name("reset_pane_name"),
            Some(WmAction::ResetPaneName)
        );
        assert_eq!(
            action_from_name("reset_workspace_name"),
            Some(WmAction::ResetWorkspaceName)
        );
        assert_eq!(action_from_name("close"), Some(WmAction::ClosePane));
        assert_eq!(
            action_from_name("command_palette"),
            Some(WmAction::CommandPalette {
                mode: None,
                query: None
            })
        );
        // Font zoom — six names map to two variants with the right step + None pane.
        assert_eq!(
            action_from_name("app_font_increase"),
            Some(WmAction::AppFontZoom {
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            action_from_name("app_font_reset"),
            Some(WmAction::AppFontZoom {
                step: FontZoomStep::Reset
            })
        );
        assert_eq!(
            action_from_name("pane_terminal_font_decrease"),
            Some(WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::Out
            })
        );
        // Selection actions are generic (host capability), not terminal-only.
        assert_eq!(
            action_from_name("enter_selection_mode"),
            Some(WmAction::EnterSelectionMode)
        );
        assert_eq!(
            action_from_name("selection_left"),
            Some(WmAction::SelectionLeft)
        );
        assert_eq!(
            action_from_name("selection_right"),
            Some(WmAction::SelectionRight)
        );
        assert_eq!(
            action_from_name("selection_up"),
            Some(WmAction::SelectionUp)
        );
        assert_eq!(
            action_from_name("selection_down"),
            Some(WmAction::SelectionDown)
        );
        assert_eq!(
            action_from_name("clear_selection"),
            Some(WmAction::ClearSelection)
        );
        assert_eq!(
            action_from_name("copy_selection"),
            Some(WmAction::CopySelection)
        );
        assert_eq!(
            action_from_name("paste_clipboard"),
            Some(WmAction::PasteClipboard)
        );
        assert_eq!(
            action_from_name("begin_selection"),
            Some(WmAction::BeginSelection)
        );
        assert_eq!(
            action_from_name("toggle_selection_endpoint"),
            Some(WmAction::ToggleSelectionEndpoint)
        );
        // Selection has no parameterized variants in Task 02; the parameterized
        // pathway is unreachable by design.
        assert_eq!(action_from_name("copy_selection_42"), None);
        assert_eq!(action_from_name("enter_selection_mode_now"), None);
    }

    #[test]
    fn test_action_from_name_unknown() {
        assert_eq!(action_from_name("not_real"), None);
        assert_eq!(action_from_name(""), None);
    }

    #[test]
    fn test_scrollback_action_from_name() {
        assert_eq!(
            action_from_name("scrollback_page_up"),
            Some(WmAction::ScrollbackPageUp)
        );
        assert_eq!(
            action_from_name("scrollback_page_down"),
            Some(WmAction::ScrollbackPageDown)
        );
        assert_eq!(
            action_from_name("scrollback_to_top"),
            Some(WmAction::ScrollbackToTop)
        );
        assert_eq!(
            action_from_name("scrollback_to_bottom"),
            Some(WmAction::ScrollbackToBottom)
        );
        assert_eq!(
            action_from_name("exit_scrollback"),
            Some(WmAction::ExitScrollback)
        );
        assert_eq!(
            action_from_name("scrollback_line_up"),
            Some(WmAction::ScrollbackLineUp { amount: 1 })
        );
        assert_eq!(
            action_from_name("scrollback_line_down"),
            Some(WmAction::ScrollbackLineDown { amount: 1 })
        );
    }

    #[test]
    fn test_focus_workspace_dynamic_parsing() {
        assert_eq!(
            action_from_name("focus_workspace_1"),
            Some(WmAction::FocusWorkspace { ws_idx: 0 })
        );
        assert_eq!(
            action_from_name("focus_workspace_9"),
            Some(WmAction::FocusWorkspace { ws_idx: 8 })
        );
        assert_eq!(action_from_name("focus_workspace_0"), None);
        assert_eq!(action_from_name("focus_workspace_"), None);
        assert_eq!(action_from_name("focus_workspace"), None);
    }
}
