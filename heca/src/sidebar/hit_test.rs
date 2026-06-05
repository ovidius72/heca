use crate::input::WmAction;

use super::{SidebarItem, SidebarTree};

const ITEM_HEIGHT: f32 = 24.0;
pub(crate) const BTN_ROW_HEIGHT: f32 = ITEM_HEIGHT;

/// Hit-test the sidebar to find which flat item (if any) is under `mouse_y`.
///
/// * `sidebar_top` — Y coordinate of the sidebar's top edge
/// * `sidebar_height` — total height of the sidebar content area
/// * `sidebar_width` — current width (used to decide expanded vs collapsed)
/// * `mouse_y` — the mouse cursor's Y coordinate
///
/// Returns the flat item index, or `None` if the click missed all items.
pub fn sidebar_hit_test(
    tree: &SidebarTree,
    sidebar_top: f32,
    sidebar_height: f32,
    sidebar_width: f32,
    mouse_y: f32,
) -> Option<usize> {
    if mouse_y < sidebar_top || mouse_y > sidebar_top + sidebar_height {
        return None;
    }

    let is_collapsed = sidebar_width < 80.0;
    let relative_y = mouse_y - (sidebar_top + 4.0);
    if relative_y < 0.0 {
        return None;
    }

    let adjusted_y = relative_y - BTN_ROW_HEIGHT;
    if adjusted_y < 0.0 {
        return None;
    }

    let line_index = (adjusted_y / ITEM_HEIGHT) as usize;

    if is_collapsed {
        let mut visible_line = 0usize;
        for (fi, item) in tree.flat_items.iter().enumerate() {
            if matches!(item, SidebarItem::Column { .. }) {
                continue;
            }
            if visible_line == line_index {
                return Some(fi);
            }
            visible_line += 1;
        }
    } else {
        let visible_lines = (sidebar_height / ITEM_HEIGHT) as usize;
        let fi = tree.scroll_offset + line_index;
        if line_index < visible_lines && fi < tree.flat_items.len() {
            return tree.flat_items.get(fi).map(|_| fi);
        }
    }

    None
}

/// Check if a mouse position hits any sidebar button.
/// Returns the button if hit, None otherwise.
pub fn sidebar_button_hit_test(
    tree: &SidebarTree,
    mouse_x: f32,
    mouse_y: f32,
) -> Option<(usize, WmAction)> {
    for (i, hitbox) in tree.button_hitboxes.iter().enumerate() {
        if mouse_x >= hitbox.x
            && mouse_x <= hitbox.x + hitbox.width
            && mouse_y >= hitbox.y
            && mouse_y <= hitbox.y + hitbox.height
        {
            return Some((i, hitbox.action.clone()));
        }
    }
    None
}
