use super::WorkspaceTree;

pub(crate) const ITEM_HEIGHT: f32 = 24.0;
pub(crate) const BTN_ROW_HEIGHT: f32 = ITEM_HEIGHT;

/// Hit-test the **expanded** sidebar to find which flat item (if any) is under
/// `mouse_y`. Used by the sidebar drag surface (hover / `item_at`). There is no
/// collapsed rail — a region is Expanded ⇄ Hidden (see
/// `docs/sidebar-provider-modes.md`); a Hidden sidebar has width 0, so the caller's
/// bounds check keeps `mouse_y` out and this is never asked about it.
///
/// * `sidebar_top` — Y coordinate of the sidebar's top edge
/// * `sidebar_height` — total height of the sidebar content area
/// * `mouse_y` — the mouse cursor's Y coordinate
///
/// Returns the flat item index, or `None` if the click missed all items.
pub fn sidebar_hit_test(
    tree: &WorkspaceTree,
    sidebar_top: f32,
    sidebar_height: f32,
    mouse_y: f32,
) -> Option<usize> {
    if mouse_y < sidebar_top || mouse_y > sidebar_top + sidebar_height {
        return None;
    }

    let relative_y = mouse_y - (sidebar_top + 4.0);
    if relative_y < 0.0 {
        return None;
    }

    // A `[+w]` button row (`BTN_ROW_HEIGHT`) precedes the first item.
    let adjusted_y = relative_y - BTN_ROW_HEIGHT;
    if adjusted_y < 0.0 {
        return None;
    }

    let line_index = (adjusted_y / ITEM_HEIGHT) as usize;
    let visible_lines = (sidebar_height / ITEM_HEIGHT) as usize;
    let fi = tree.scroll_offset + line_index;
    if line_index < visible_lines && fi < tree.flat_items.len() {
        return tree.flat_items.get(fi).map(|_| fi);
    }

    None
}
