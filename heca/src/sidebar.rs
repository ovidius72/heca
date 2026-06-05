mod hit_test;
mod model;

use crate::input::WmAction;

#[cfg(test)]
use hit_test::BTN_ROW_HEIGHT;
pub use hit_test::{sidebar_button_hit_test, sidebar_hit_test};
#[allow(unused_imports)]
pub use model::{
    SidebarButtonHitbox, SidebarColEntry, SidebarItem, SidebarPaneEntry, SidebarTree,
    SidebarWsEntry,
};

// ---------------------------------------------------------------------------
// Sidebar rendering functions
// ---------------------------------------------------------------------------

use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;

const ITEM_HEIGHT: f32 = 24.0;
const INDENT_WS: f32 = 8.0;
const INDENT_COL: f32 = 26.0;
const INDENT_PANE: f32 = 44.0;
const BTN_SIZE: f32 = 20.0;
const BTN_PAD_X: f32 = 2.0;
const BTN_RADIUS: f32 = 4.0;

/// Render the expanded sidebar tree (width >= 80px).
/// If `candidates` is provided, pane letters are shown during PaneSelect/PaneSwap.
// Each param is a distinct render input; grouping would hurt call-site readability.
#[allow(clippy::too_many_arguments)]
pub fn render_sidebar_expanded(
    tree: &mut SidebarTree,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    is_sidebar_nav: bool,
    accent: [f32; 4],
    foreground: [f32; 4],
    cursor_bg: [f32; 4],
    visited_color: [f32; 4],
    candidates: Option<&[(char, u64)]>,
    focused_pane: Option<u64>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    // Flat index being hovered during drag (for highlight).
    drag_hover_fi: Option<usize>,
    // Flat index of the item being dragged (for drag source visual effect).
    drag_source_fi: Option<usize>,
    // Drag source colors from theme.
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
    // Hovered button index for hover effect.
    hovered_btn_idx: Option<usize>,
    // Font sizes from theme.
    label_font_size: f32,
    button_font_size: f32,
) {
    // Clear and collect button hitboxes.
    tree.button_hitboxes.clear();

    let scroll = tree.scroll_offset;
    let mut line_y = y + 4.0;
    let visible_lines = (height / ITEM_HEIGHT) as usize;
    let mut drawn = 0usize;

    // [+w] button at the top of the sidebar.
    {
        let btn_x = x + width - BTN_SIZE - BTN_PAD_X - 4.0;
        let btn_y = line_y + (ITEM_HEIGHT - BTN_SIZE) / 2.0;
        let btn_idx = tree.button_hitboxes.len();
        let is_hov = hovered_btn_idx == Some(btn_idx);
        let (bg, brd, tc) = if is_hov {
            (
                [foreground[0], foreground[1], foreground[2], 0.3],
                [foreground[0], foreground[1], foreground[2], 0.7],
                foreground,
            )
        } else {
            (
                [foreground[0], foreground[1], foreground[2], 0.12],
                [foreground[0], foreground[1], foreground[2], 0.35],
                foreground,
            )
        };
        primitive_renderer
            .draw_rounded_rect(btn_x, btn_y, BTN_SIZE, BTN_SIZE, bg, brd, 1.0, BTN_RADIUS);
        let tw = 2.0 * button_font_size * 0.55;
        text_renderer.queue_text(
            "+w",
            btn_x + (BTN_SIZE - tw) / 2.0,
            btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0,
            button_font_size,
            tc,
        );
        tree.button_hitboxes.push(SidebarButtonHitbox {
            action: WmAction::CreateWorkspace,
            ws_idx: None,
            x: btn_x,
            y: btn_y,
            width: BTN_SIZE,
            height: BTN_SIZE,
        });
    }
    line_y += ITEM_HEIGHT;
    drawn += 1;

    for (fi, flat_item) in tree.flat_items.iter().enumerate() {
        if fi < scroll {
            continue;
        }
        if drawn >= visible_lines {
            break;
        }

        let is_cursor = fi == tree.cursor && is_sidebar_nav;
        let (indent, label, is_ws) = match flat_item {
            SidebarItem::Workspace { ws_idx } => {
                if let Some(ws) = tree.workspaces.get(*ws_idx) {
                    let arrow = if ws.collapsed { "▶ " } else { "▼ " };
                    (INDENT_WS, format!("{}{}", arrow, ws.name), true)
                } else {
                    (INDENT_WS, format!("WS {}", ws_idx), true)
                }
            }
            SidebarItem::Column { ws_idx, col_idx } => {
                let label = tree
                    .workspaces
                    .get(*ws_idx)
                    .and_then(|ws| ws.columns.get(*col_idx))
                    .map(|col| col.name.clone())
                    .unwrap_or_else(|| format!("Col {}", col_idx + 1));
                (INDENT_COL, label, false)
            }
            SidebarItem::Pane { pane_id } => {
                let mut label = pane_name_short(*pane_id, &tree.workspaces);
                if let Some(cands) = candidates
                    && focused_pane != Some(*pane_id)
                    && let Some((ch, _)) = cands.iter().find(|(_, pid)| *pid == *pane_id)
                {
                    label = format!("[{}] {}", ch, label);
                }
                (INDENT_PANE, label, false)
            }
            SidebarItem::FloatingPane { pane_id, .. } => {
                let mut label = format!("~ {}", pane_name_short(*pane_id, &tree.workspaces));
                if let Some(cands) = candidates
                    && focused_pane != Some(*pane_id)
                    && let Some((ch, _)) = cands.iter().find(|(_, pid)| *pid == *pane_id)
                {
                    label = format!("[{}] {}", ch, label);
                }
                (INDENT_PANE, label, false)
            }
        };

        // Cursor background (sidebar nav mode)
        if is_cursor {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, cursor_bg);
        }

        // Drag hover highlight (lighter accent)
        if drag_hover_fi == Some(fi) {
            let mut drag_bg = accent;
            drag_bg[3] = 0.25; // more transparent than cursor
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_bg);
        }

        // Drag source effect — the item being dragged gets a themed background + border.
        if drag_source_fi == Some(fi) {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_source_bg);
            primitive_renderer.draw_border(
                x + 1.0,
                line_y + 1.0,
                width - 2.0,
                ITEM_HEIGHT - 2.0,
                drag_source_border,
                1.0,
            );
        }

        // Determine color based on state
        let color = if is_cursor {
            accent
        } else if is_ws {
            // Look up workspace state
            if let Some(ws) = flat_item
                .workspace_idx()
                .and_then(|wi| tree.workspaces.get(wi))
            {
                match ws.state {
                    crate::app_state::SidebarItemState::Active => accent,
                    crate::app_state::SidebarItemState::Visited => visited_color,
                    crate::app_state::SidebarItemState::None => foreground,
                }
            } else {
                foreground
            }
        } else {
            match flat_item {
                SidebarItem::Pane { pane_id } | SidebarItem::FloatingPane { pane_id, .. } => {
                    pane_entry_by_id(*pane_id, &tree.workspaces)
                        .map(|pane| match pane.state {
                            crate::app_state::SidebarItemState::Active => accent,
                            crate::app_state::SidebarItemState::Visited => visited_color,
                            crate::app_state::SidebarItemState::None => foreground,
                        })
                        .unwrap_or(foreground)
                }
                _ => foreground,
            }
        };

        // Pane background: active gets a prominent tint, visited gets a subtle tint.
        if let SidebarItem::Pane { pane_id } | SidebarItem::FloatingPane { pane_id, .. } = flat_item
            && let Some(pane) = pane_entry_by_id(*pane_id, &tree.workspaces)
        {
            match pane.state {
                crate::app_state::SidebarItemState::Active => {
                    let mut bg = accent;
                    bg[3] = 0.18;
                    primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, bg);
                }
                crate::app_state::SidebarItemState::Visited => {
                    let mut bg = visited_color;
                    bg[3] = 0.10;
                    primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, bg);
                }
                crate::app_state::SidebarItemState::None => {}
            }
        }

        let text_x = x + indent;
        let text_y = line_y + (ITEM_HEIGHT - label_font_size) / 2.0 + 2.0;
        text_renderer.queue_text(&label, text_x, text_y, label_font_size, color);

        // Buttons on the right side of each item.
        // Workspace and column items get two buttons (add + delete), pane gets one (delete).
        let btn_x_right = x + width - BTN_SIZE - BTN_PAD_X - 4.0;
        let btn_y = line_y + (ITEM_HEIGHT - BTN_SIZE) / 2.0;

        if let SidebarItem::Workspace { ws_idx } = flat_item {
            // Add button [+c]
            let add_x = btn_x_right - BTN_SIZE - BTN_PAD_X;
            let add_idx = tree.button_hitboxes.len();
            let add_hov = hovered_btn_idx == Some(add_idx);
            let (abg, abrd, atc) = if add_hov {
                (
                    [foreground[0], foreground[1], foreground[2], 0.3],
                    [foreground[0], foreground[1], foreground[2], 0.7],
                    foreground,
                )
            } else {
                (
                    [foreground[0], foreground[1], foreground[2], 0.12],
                    [foreground[0], foreground[1], foreground[2], 0.35],
                    foreground,
                )
            };
            primitive_renderer
                .draw_rounded_rect(add_x, btn_y, BTN_SIZE, BTN_SIZE, abg, abrd, 1.0, BTN_RADIUS);
            let tw = 2.0 * button_font_size * 0.55;
            text_renderer.queue_text(
                "+c",
                add_x + (BTN_SIZE - tw) / 2.0,
                btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0,
                button_font_size,
                atc,
            );
            tree.button_hitboxes.push(SidebarButtonHitbox {
                action: WmAction::SplitHorizontal,
                ws_idx: Some(*ws_idx),
                x: add_x,
                y: btn_y,
                width: BTN_SIZE,
                height: BTN_SIZE,
            });

            // Delete button [-]
            let del_idx = tree.button_hitboxes.len();
            let del_hov = hovered_btn_idx == Some(del_idx);
            let (dbg, dbrd, dtc) = if del_hov {
                (
                    [0.9, 0.3, 0.3, 0.4],
                    [0.9, 0.3, 0.3, 0.8],
                    [0.95, 0.4, 0.4, 1.0],
                )
            } else {
                (
                    [0.9, 0.3, 0.3, 0.15],
                    [0.9, 0.3, 0.3, 0.4],
                    [0.9, 0.3, 0.3, 0.9],
                )
            };
            primitive_renderer.draw_rounded_rect(
                btn_x_right,
                btn_y,
                BTN_SIZE,
                BTN_SIZE,
                dbg,
                dbrd,
                1.0,
                BTN_RADIUS,
            );
            let dw = 1.0 * button_font_size * 0.55;
            text_renderer.queue_text(
                "-",
                btn_x_right + (BTN_SIZE - dw) / 2.0,
                btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0,
                button_font_size,
                dtc,
            );
            tree.button_hitboxes.push(SidebarButtonHitbox {
                action: WmAction::DeleteWorkspace { ws_idx: *ws_idx },
                ws_idx: Some(*ws_idx),
                x: btn_x_right,
                y: btn_y,
                width: BTN_SIZE,
                height: BTN_SIZE,
            });
        }

        if let SidebarItem::Column { ws_idx, col_idx } = flat_item {
            // Add button [+p]
            let add_x = btn_x_right - BTN_SIZE - BTN_PAD_X;
            let add_idx = tree.button_hitboxes.len();
            let add_hov = hovered_btn_idx == Some(add_idx);
            let (abg, abrd, atc) = if add_hov {
                (
                    [foreground[0], foreground[1], foreground[2], 0.3],
                    [foreground[0], foreground[1], foreground[2], 0.7],
                    foreground,
                )
            } else {
                (
                    [foreground[0], foreground[1], foreground[2], 0.12],
                    [foreground[0], foreground[1], foreground[2], 0.35],
                    foreground,
                )
            };
            primitive_renderer
                .draw_rounded_rect(add_x, btn_y, BTN_SIZE, BTN_SIZE, abg, abrd, 1.0, BTN_RADIUS);
            let tw = 2.0 * button_font_size * 0.55;
            text_renderer.queue_text(
                "+p",
                add_x + (BTN_SIZE - tw) / 2.0,
                btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0,
                button_font_size,
                atc,
            );
            tree.button_hitboxes.push(SidebarButtonHitbox {
                action: WmAction::AddPaneToColumn {
                    ws_idx: *ws_idx,
                    col_idx: *col_idx,
                },
                ws_idx: Some(*ws_idx),
                x: add_x,
                y: btn_y,
                width: BTN_SIZE,
                height: BTN_SIZE,
            });

            // Delete button [-]
            let del_idx = tree.button_hitboxes.len();
            let del_hov = hovered_btn_idx == Some(del_idx);
            let (dbg, dbrd, dtc) = if del_hov {
                (
                    [0.9, 0.3, 0.3, 0.4],
                    [0.9, 0.3, 0.3, 0.8],
                    [0.95, 0.4, 0.4, 1.0],
                )
            } else {
                (
                    [0.9, 0.3, 0.3, 0.15],
                    [0.9, 0.3, 0.3, 0.4],
                    [0.9, 0.3, 0.3, 0.9],
                )
            };
            primitive_renderer.draw_rounded_rect(
                btn_x_right,
                btn_y,
                BTN_SIZE,
                BTN_SIZE,
                dbg,
                dbrd,
                1.0,
                BTN_RADIUS,
            );
            let dw = 1.0 * button_font_size * 0.55;
            text_renderer.queue_text(
                "-",
                btn_x_right + (BTN_SIZE - dw) / 2.0,
                btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0,
                button_font_size,
                dtc,
            );
            tree.button_hitboxes.push(SidebarButtonHitbox {
                action: WmAction::DeleteColumn {
                    ws_idx: *ws_idx,
                    col_idx: *col_idx,
                },
                ws_idx: Some(*ws_idx),
                x: btn_x_right,
                y: btn_y,
                width: BTN_SIZE,
                height: BTN_SIZE,
            });
        }

        if let SidebarItem::Pane { pane_id } = flat_item {
            let del_idx = tree.button_hitboxes.len();
            let del_hov = hovered_btn_idx == Some(del_idx);
            let (dbg, dbrd, dtc) = if del_hov {
                (
                    [0.9, 0.3, 0.3, 0.4],
                    [0.9, 0.3, 0.3, 0.8],
                    [0.95, 0.4, 0.4, 1.0],
                )
            } else {
                (
                    [0.9, 0.3, 0.3, 0.15],
                    [0.9, 0.3, 0.3, 0.4],
                    [0.9, 0.3, 0.3, 0.9],
                )
            };
            primitive_renderer.draw_rounded_rect(
                btn_x_right,
                btn_y,
                BTN_SIZE,
                BTN_SIZE,
                dbg,
                dbrd,
                1.0,
                BTN_RADIUS,
            );
            let dw = 1.0 * button_font_size * 0.55;
            text_renderer.queue_text(
                "-",
                btn_x_right + (BTN_SIZE - dw) / 2.0,
                btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0,
                button_font_size,
                dtc,
            );
            tree.button_hitboxes.push(SidebarButtonHitbox {
                action: WmAction::ClosePaneById { pane_id: *pane_id },
                ws_idx: None,
                x: btn_x_right,
                y: btn_y,
                width: BTN_SIZE,
                height: BTN_SIZE,
            });
        }

        line_y += ITEM_HEIGHT;
        drawn += 1;
    }
}

/// Render the collapsed sidebar (activity strip at 40px width).
/// When `is_sidebar_nav` is true, the cursor line is highlighted so the user
/// can navigate even in collapsed mode.
/// `candidates` are shown as pane letters during PaneSwap / PaneSelect.
// Each param is a distinct render input; grouping would hurt call-site readability.
#[allow(clippy::too_many_arguments)]
pub fn render_sidebar_collapsed(
    tree: &mut SidebarTree,
    x: f32,
    y: f32,
    width: f32,
    _height: f32,
    is_sidebar_nav: bool,
    accent: [f32; 4],
    foreground: [f32; 4],
    visited_color: [f32; 4],
    cursor_bg: [f32; 4],
    candidates: Option<&[(char, u64)]>,
    focused_pane: Option<u64>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    // Flat index being hovered during drag (for highlight).
    drag_hover_fi: Option<usize>,
    // Flat index of the item being dragged (for drag source visual effect).
    drag_source_fi: Option<usize>,
    // Drag source colors from theme.
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
    // Hovered button index (unused in collapsed — no buttons).
    _hovered_btn_idx: Option<usize>,
    // Font sizes from theme.
    label_font_size: f32,
    _button_font_size: f32,
) {
    let font_size = label_font_size;
    let activity_bar_w = 4.0;
    let text_x = x + activity_bar_w + 4.0; // 4px gap after activity bar
    let mut line_y = y + 4.0;

    // Track flat item index so we can match the cursor position.
    // flat_items ordering: Workspace, [Column, [Pane...]...]...
    let mut flat_idx = 0usize;

    for ws in &tree.workspaces {
        let is_ws_cursor = is_sidebar_nav && flat_idx == tree.cursor;
        flat_idx += 1; // consume Workspace item

        // Activity bar
        let bar_color = match ws.state {
            crate::app_state::SidebarItemState::Active => accent,
            crate::app_state::SidebarItemState::Visited => visited_color,
            crate::app_state::SidebarItemState::None => [0.0; 4], // transparent
        };

        // Count lines for this workspace.
        let section_lines = 1 + if ws.collapsed {
            0
        } else {
            ws.columns.iter().map(|c| c.panes.len()).sum::<usize>() + ws.floating_panes.len()
        };
        let section_height = section_lines as f32 * ITEM_HEIGHT;

        // Draw activity bar (full height of section)
        if bar_color[3] > 0.0 {
            primitive_renderer.draw_rect(x, line_y, activity_bar_w, section_height, bar_color);
        }

        // Cursor highlight
        if is_ws_cursor {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, cursor_bg);
        }

        // Drag hover highlight (workspace)
        if drag_hover_fi == Some(flat_idx - 1) {
            let mut drag_bg = accent;
            drag_bg[3] = 0.25;
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_bg);
        }

        // Drag source effect for workspace items.
        if drag_source_fi == Some(flat_idx - 1) {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_source_bg);
            primitive_renderer.draw_border(
                x + 1.0,
                line_y + 1.0,
                width - 2.0,
                ITEM_HEIGHT - 2.0,
                drag_source_border,
                1.0,
            );
        }

        // Workspace number/identifier (first 2 chars)
        let ws_label = ws.name.chars().take(2).collect::<String>();
        let ws_color = if is_ws_cursor {
            accent
        } else {
            match ws.state {
                crate::app_state::SidebarItemState::Active => accent,
                crate::app_state::SidebarItemState::Visited => visited_color,
                crate::app_state::SidebarItemState::None => foreground,
            }
        };
        let ws_text_y = line_y + (ITEM_HEIGHT - font_size) / 2.0;
        text_renderer.queue_text(&ws_label, text_x, ws_text_y, font_size, ws_color);
        line_y += ITEM_HEIGHT;

        // Pane letters
        if !ws.collapsed {
            for col in &ws.columns {
                flat_idx += 1; // consume Column item (invisible in collapsed mode)
                for pane in &col.panes {
                    let item_idx = flat_idx;
                    let is_pane_cursor = is_sidebar_nav && item_idx == tree.cursor;
                    flat_idx += 1; // consume Pane item

                    if is_pane_cursor {
                        primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, cursor_bg);
                    }

                    // Drag hover highlight (pane or column)
                    if drag_hover_fi == Some(item_idx) {
                        let mut drag_bg = accent;
                        drag_bg[3] = 0.25;
                        primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_bg);
                    }

                    // Drag source effect for pane items.
                    if drag_source_fi == Some(item_idx) {
                        primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_source_bg);
                        primitive_renderer.draw_border(
                            x + 1.0,
                            line_y + 1.0,
                            width - 2.0,
                            ITEM_HEIGHT - 2.0,
                            drag_source_border,
                            1.0,
                        );
                    }

                    let mut pane_char = pane.name.chars().next().unwrap_or('?').to_string();
                    // Show candidate letter during PaneSwap / PaneSelect
                    // but NOT for the focused pane — no need to swap with yourself.
                    if let Some(cands) = candidates
                        && focused_pane != Some(pane.pane_id)
                        && let Some((ch, _)) = cands.iter().find(|(_, pid)| *pid == pane.pane_id)
                    {
                        pane_char = ch.to_string();
                    }

                    let pane_color = if is_pane_cursor {
                        accent
                    } else {
                        match pane.state {
                            crate::app_state::SidebarItemState::Active => accent,
                            crate::app_state::SidebarItemState::Visited => visited_color,
                            crate::app_state::SidebarItemState::None => foreground,
                        }
                    };
                    let pane_text_y = line_y + (ITEM_HEIGHT - font_size) / 2.0;
                    text_renderer.queue_text(
                        &pane_char,
                        text_x,
                        pane_text_y,
                        font_size,
                        pane_color,
                    );
                    line_y += ITEM_HEIGHT;
                }
            }

            for pane in &ws.floating_panes {
                let item_idx = flat_idx;
                flat_idx += 1; // consume FloatingPane item

                if drag_hover_fi == Some(item_idx) {
                    let mut drag_bg = accent;
                    drag_bg[3] = 0.18;
                    primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_bg);
                }

                let mut pane_char = pane.name.chars().next().unwrap_or('~').to_string();
                if let Some(cands) = candidates
                    && focused_pane != Some(pane.pane_id)
                    && let Some((ch, _)) = cands.iter().find(|(_, pid)| *pid == pane.pane_id)
                {
                    pane_char = ch.to_string();
                }

                let pane_color = match pane.state {
                    crate::app_state::SidebarItemState::Active => accent,
                    crate::app_state::SidebarItemState::Visited => visited_color,
                    crate::app_state::SidebarItemState::None => foreground,
                };
                let pane_text_y = line_y + (ITEM_HEIGHT - font_size) / 2.0;
                text_renderer.queue_text(&pane_char, text_x, pane_text_y, font_size, pane_color);
                line_y += ITEM_HEIGHT;
            }
        }
    }
}

fn pane_entry_by_id(pane_id: u64, workspaces: &[SidebarWsEntry]) -> Option<&SidebarPaneEntry> {
    for ws in workspaces {
        for col in &ws.columns {
            if let Some(pane) = col.panes.iter().find(|pane| pane.pane_id == pane_id) {
                return Some(pane);
            }
        }
        if let Some(pane) = ws
            .floating_panes
            .iter()
            .find(|pane| pane.pane_id == pane_id)
        {
            return Some(pane);
        }
    }
    None
}

/// Helper to find a pane's display name from the tree.
fn pane_name_short(pane_id: u64, workspaces: &[SidebarWsEntry]) -> String {
    pane_entry_by_id(pane_id, workspaces)
        .map(|pane| pane.name.clone())
        .unwrap_or_else(|| format!("Pane {}", pane_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::SidebarItemState;
    use heca_core::layout::{
        Pane as LayoutPane, PaneId,
        session::Session,
        types::{SessionId, Size},
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
}
