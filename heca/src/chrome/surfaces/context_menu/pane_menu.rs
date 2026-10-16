//! **The content pane's own menu** — the app's domain, so the one menu the host itself builds.

use super::ContextTarget;
use super::entries::item_running;
use crate::chrome::{DropdownItem, PropValue};
use crate::providers::ChromeCtx;

/// Pane context-menu items. `has_custom_name` gates the "Use process name" reset entry — it only
/// appears when there is a custom name to clear. This is a **pane** menu, so it carries no
/// workspace-reset action.
pub(super) fn pane_action_items(has_custom_name: bool) -> Vec<DropdownItem> {
    let mut items = vec![
        DropdownItem::new("split_horizontal", "New column"),
        // "Add a pane" reads consistently everywhere: same label + FolderSimplePlus icon (via the
        // `add_pane_to_column` id) as the sidebar "New pane" and the pane-header "+" button. It
        // still RUNS `split_vertical` (add a pane to the active column) — identity vs behaviour.
        item_running("add_pane_to_column", "New pane", "split_vertical", &[]),
        DropdownItem::new("zoom_column", "Zoom / unzoom"),
        DropdownItem::new("float", "Float / unfloat"),
        item_running("rename_pane", "Rename", "rename_pane", &[]),
    ];
    if has_custom_name {
        items.push(DropdownItem::new("reset_pane_name", "Use process name"));
    }
    items.push(DropdownItem::new("rename_workspace", "Rename workspace"));
    items.push(item_running("close", "Close pane", "close", &[]).danger(true));
    items
}

/// Provider for [`ContextPath::PANE`](super::ContextPath::PANE) — a content pane. "Open link" first only when the mouse
/// click carried a hyperlink (keyboard-opened menus have `None` → no link entry).
pub(super) fn build_pane_menu(ctx: &ChromeCtx, target: &ContextTarget) -> Vec<DropdownItem> {
    let ContextTarget::Pane { hyperlink, pane_id } = target else {
        return Vec::new();
    };
    // Read through the facade — the same selector a plugin would use.
    let has_custom_name = ctx.state().pane_custom_name(*pane_id).is_some();
    let mut items = pane_action_items(has_custom_name);
    if let Some(url) = hyperlink {
        items.insert(
            0,
            item_running(
                "open_link",
                "Open link",
                "open_link",
                &[("url", PropValue::Text(url.clone()))],
            ),
        );
    }
    items
}
