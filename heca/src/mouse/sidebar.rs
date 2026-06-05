//! Mouse sidebar interaction helpers.
//!
//! This module owns sidebar click routing.

use crate::app_state::AppState;
use crate::input::WmAction;

pub(super) fn click(state: &mut AppState, pos: (f32, f32)) -> Option<WmAction> {
    let (_win_w, win_h) = super::window_logical_size(state);
    let chrome = super::chrome_config(state);
    let sidebar_top = chrome.tab_bar_height;
    let sidebar_bottom = win_h - chrome.status_bar_height;

    let sw = if state.sidebar.left_visible {
        chrome.left_sidebar_width
    } else {
        40.0
    };
    if pos.0 >= 0.0 && pos.0 <= sw && pos.1 >= sidebar_top && pos.1 <= sidebar_bottom {
        if let Some((btn_idx, button)) =
            crate::sidebar::sidebar_button_hit_test(&state.sidebar_tree, pos.0, pos.1)
        {
            let is_delete = matches!(
                &button,
                WmAction::DeleteWorkspace { .. } | WmAction::DeleteColumn { .. }
            );
            if is_delete {
                let message = match &button {
                    WmAction::DeleteWorkspace { ws_idx } => {
                        let ws_label = if let Some(ws) = state.session.workspaces.get(*ws_idx)
                            && let Some(ref name) = ws.name
                        {
                            name.clone()
                        } else {
                            format!("workspace {}", ws_idx + 1)
                        };
                        format!("Delete {}? (y/n)", ws_label)
                    }
                    WmAction::DeleteColumn { ws_idx, col_idx } => {
                        let ws_label = if let Some(ws) = state.session.workspaces.get(*ws_idx)
                            && let Some(ref name) = ws.name
                        {
                            name.clone()
                        } else {
                            format!("ws {}", ws_idx + 1)
                        };
                        format!("Delete column {} from {}? (y/n)", col_idx + 1, ws_label)
                    }
                    _ => unreachable!(),
                };
                state.input_mode = crate::app_state::InputMode::ConfirmDelete {
                    message,
                    action: Box::new(button),
                };
                return None;
            }

            if let Some(hitbox) = state.sidebar_tree.button_hitboxes.get(btn_idx)
                && let Some(ws_idx) = hitbox.ws_idx
                && ws_idx != state.session.active_workspace_idx
            {
                crate::switch_workspace_tracked(state, ws_idx);
            }
            return Some(button);
        }

        let sidebar_h = sidebar_bottom - sidebar_top;
        if let Some(fi) =
            crate::sidebar::sidebar_hit_test(&state.sidebar_tree, sidebar_top, sidebar_h, sw, pos.1)
        {
            state.sidebar_tree.cursor = fi;
            let item = state.sidebar_tree.current_item().cloned();
            match item? {
                crate::sidebar::SidebarItem::Pane { pane_id } => {
                    return Some(WmAction::FocusPane { pane_id });
                }
                crate::sidebar::SidebarItem::Workspace { ws_idx } => {
                    return Some(WmAction::FocusWorkspace { ws_idx });
                }
                crate::sidebar::SidebarItem::Column { .. } => {}
                crate::sidebar::SidebarItem::FloatingPane { .. } => {}
            }
        }
    }

    None
}
