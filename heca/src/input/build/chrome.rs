//! Actions on what is drawn around the panes: containers, docks, layers, the palette, search history, overlays, regions, notifications.

use super::{get_enum, get_enum_or_default, get_string, get_u64};
use crate::input::WmAction;

pub(super) fn build_chrome(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    match name {
        "cursor_to" => Some(WmAction::CursorTo {
            mount: args.get("mount")?.clone(),
            key: args.get("key")?.clone(),
        }),
        "notification_dismiss_one" => Some(WmAction::NotificationDismissOne {
            notification_id: get_u64(args, "id")?,
        }),
        "notification_action_relay" => Some(WmAction::NotificationActionRelay {
            notification_id: get_u64(args, "id")?,
            key: get_string(args, "key")?,
        }),
        "chrome.container.move_to_region" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: get_enum(args, "region")?,
        }),
        // Conveniences over move_to_region with the region fixed: what a menu item or a keybinding
        // ("send this container to the right sidebar") actually wants to say.
        "chrome.container.move_left_sidebar" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: crate::chrome::RegionId::LeftSidebar,
        }),
        "chrome.container.move_right_sidebar" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: crate::chrome::RegionId::RightSidebar,
        }),
        // `before_id` is OPTIONAL: omitting it moves the container to the END of its region.
        "chrome.container.reorder_before" => Some(WmAction::ReorderContainerBefore {
            container_id: get_string(args, "container_id")?,
            before_id: get_string(args, "before_id"),
        }),
        "chrome.container.reorder_after" => Some(WmAction::ReorderContainerAfter {
            container_id: get_string(args, "container_id")?,
            after_id: get_string(args, "after_id")?,
        }),
        // `dock` is OPTIONAL, which is what makes one action serve both doors: a keybinding cannot
        // name a container, so a bare binding picks one by letter; a caller that knows which dock it
        // wants says so and skips the pick. An action with a *required* argument could not be bound
        // bare at all (F003/P010/T006).
        "focus_dock" => Some(WmAction::FocusDock {
            dock: get_string(args, "dock"),
        }),
        "toggle_dock" => Some(WmAction::ToggleDock {
            dock: get_string(args, "dock"),
        }),

        "show_layer" => Some(WmAction::ShowLayer {
            name: get_string(args, "name"),
            dock: get_string(args, "dock"),
        }),
        "hide_layer" => Some(WmAction::HideLayer {
            name: get_string(args, "name"),
            dock: get_string(args, "dock"),
        }),
        "toggle_layer" => Some(WmAction::ToggleLayer {
            name: get_string(args, "name"),
            dock: get_string(args, "dock"),
        }),

        // Both OPTIONAL, and they compose into one prefilled query — see the variant.
        "command_palette" => Some(WmAction::CommandPalette {
            mode: get_string(args, "mode"),
            query: get_string(args, "query"),
        }),

        "clear_search_history" => Some(WmAction::ClearSearchHistory {
            scope: get_string(args, "scope"),
        }),
        "clear_search_ranking" => Some(WmAction::ClearSearchRanking {
            scope: get_string(args, "scope"),
        }),

        "submit_overlay" => Some(WmAction::SubmitOverlay {
            overlay: None,
            action: get_string(args, "action")?,
        }),
        "set_region_visible" => Some(WmAction::SetRegionVisible {
            region: get_enum(args, "region")?,
            visible: get_enum_or_default(args, "visible")?,
        }),

        _ => None,
    }
}
