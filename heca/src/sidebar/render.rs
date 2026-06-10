use crate::app_state::SidebarItemState;
use crate::input::WmAction;
use heca_grid_ui::drag::DragItemId;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_core::layout::PaneId;
use heca_renderer::text::TextRenderer;

use super::{
    SidebarButtonHitbox, SidebarItem, SidebarItemKind, SidebarPaneEntry, SidebarTree,
    SidebarWsEntry,
};

pub(crate) const ITEM_HEIGHT: f32 = 24.0;
const INDENT_WS: f32 = 8.0;
const INDENT_COL: f32 = 26.0;
const INDENT_PANE: f32 = 44.0;
const BTN_SIZE: f32 = 20.0;
const BTN_PAD_X: f32 = 2.0;
const BTN_RADIUS: f32 = 4.0;

#[derive(Clone, Copy)]
struct RenderColors {
    accent: [f32; 4],
    foreground: [f32; 4],
    cursor_bg: [f32; 4],
    visited_color: [f32; 4],
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
}

#[derive(Clone, Copy)]
struct RenderFonts {
    label: f32,
    button: f32,
}

#[derive(Clone, Copy)]
struct ButtonColors {
    bg: [f32; 4],
    border: [f32; 4],
    text: [f32; 4],
}

#[derive(Clone, Copy)]
struct ButtonPlacement {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
struct ExpandedItemPresentation {
    indent: f32,
}

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
    candidates: Option<&[(char, PaneId)]>,
    focused_pane: Option<PaneId>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    drag_hover: Option<DragItemId>,
    drag_source: Option<DragItemId>,
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
    hovered_btn_idx: Option<usize>,
    label_font_size: f32,
    button_font_size: f32,
) {
    tree.button_hitboxes.clear();

    let colors = RenderColors {
        accent,
        foreground,
        cursor_bg,
        visited_color,
        drag_source_bg,
        drag_source_border,
    };
    let fonts = RenderFonts {
        label: label_font_size,
        button: button_font_size,
    };

    let scroll = tree.scroll_offset;
    let mut line_y = y + 4.0;
    let visible_lines = (height / ITEM_HEIGHT) as usize;
    let mut drawn = 0usize;

    render_create_workspace_button(
        &mut tree.button_hitboxes,
        x,
        width,
        line_y,
        colors.foreground,
        hovered_btn_idx,
        fonts.button,
        text_renderer,
        primitive_renderer,
    );
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
        let (presentation, label) = expanded_item_label(
            flat_item,
            tree,
            candidates,
            focused_pane,
        );

        if is_cursor {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, colors.cursor_bg);
        }

        draw_drag_hover(primitive_renderer, x, line_y, width, colors.accent, 0.25, drag_hover == Some(DragItemId::new(fi)));
        draw_drag_source(
            primitive_renderer,
            x,
            line_y,
            width,
            colors.drag_source_bg,
            colors.drag_source_border,
            drag_source == Some(DragItemId::new(fi)),
        );

        let color = expanded_item_text_color(
            flat_item,
            is_cursor,
            tree,
            colors,
        );

        draw_expanded_pane_background(
            primitive_renderer,
            flat_item,
            x,
            line_y,
            width,
            tree,
            colors,
        );

        let text_x = x + presentation.indent;
        push_disclosure_hitbox(flat_item, &mut tree.button_hitboxes, text_x, line_y);
        let text_y = line_y + (ITEM_HEIGHT - fonts.label) / 2.0 + 2.0;
        text_renderer.queue_text(&label, text_x, text_y, fonts.label, color);

        render_expanded_item_buttons(
            flat_item,
            &mut tree.button_hitboxes,
            x,
            width,
            line_y,
            colors.foreground,
            hovered_btn_idx,
            fonts.button,
            text_renderer,
            primitive_renderer,
        );

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
    candidates: Option<&[(char, PaneId)]>,
    focused_pane: Option<PaneId>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    drag_hover: Option<DragItemId>,
    drag_source: Option<DragItemId>,
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
    _hovered_btn_idx: Option<usize>,
    label_font_size: f32,
    _button_font_size: f32,
) {
    let colors = RenderColors {
        accent,
        foreground,
        cursor_bg,
        visited_color,
        drag_source_bg,
        drag_source_border,
    };
    let font_size = label_font_size;
    let activity_bar_w = 4.0;
    let text_x = x + activity_bar_w + 4.0;
    let mut line_y = y + 4.0;
    let mut flat_idx = 0usize;

    for ws in &tree.workspaces {
        let is_ws_cursor = is_sidebar_nav && flat_idx == tree.cursor;
        flat_idx += 1;

        let section_lines = 1 + if ws.collapsed {
            0
        } else {
            ws.columns.iter().map(|c| c.panes.len()).sum::<usize>() + ws.floating_panes.len()
        };
        let section_height = section_lines as f32 * ITEM_HEIGHT;
        let workspace_fi = flat_idx - 1;

        draw_activity_bar(
            primitive_renderer,
            x,
            line_y,
            activity_bar_w,
            section_height,
            item_state_color(ws.state, colors.accent, colors.foreground, colors.visited_color),
        );

        if is_ws_cursor {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, colors.cursor_bg);
        }

        draw_drag_hover(
            primitive_renderer,
            x,
            line_y,
            width,
            colors.accent,
            0.25,
            drag_hover == Some(DragItemId::new(workspace_fi)),
        );
        draw_drag_source(
            primitive_renderer,
            x,
            line_y,
            width,
            colors.drag_source_bg,
            colors.drag_source_border,
            drag_source == Some(DragItemId::new(workspace_fi)),
        );

        let ws_label = ws.name.chars().take(2).collect::<String>();
        let ws_color = if is_ws_cursor {
            colors.accent
        } else {
            item_state_color(ws.state, colors.accent, colors.foreground, colors.visited_color)
        };
        let ws_text_y = line_y + (ITEM_HEIGHT - font_size) / 2.0;
        text_renderer.queue_text(&ws_label, text_x, ws_text_y, font_size, ws_color);
        line_y += ITEM_HEIGHT;

        if !ws.collapsed {
            render_collapsed_columns(
                ws,
                tree.cursor,
                &mut flat_idx,
                &mut line_y,
                x,
                width,
                text_x,
                font_size,
                is_sidebar_nav,
                candidates,
                focused_pane,
                colors,
                drag_hover,
                drag_source,
                text_renderer,
                primitive_renderer,
            );
            render_collapsed_floating_panes(
                ws,
                &mut flat_idx,
                &mut line_y,
                x,
                width,
                text_x,
                font_size,
                candidates,
                focused_pane,
                colors,
                drag_hover,
                text_renderer,
                primitive_renderer,
            );
        }
    }
}

fn expanded_item_label(
    flat_item: &SidebarItem,
    tree: &SidebarTree,
    candidates: Option<&[(char, PaneId)]>,
    focused_pane: Option<PaneId>,
) -> (ExpandedItemPresentation, String) {
    match flat_item {
        SidebarItem::Workspace { ws_idx } => {
            let label = if let Some(ws) = tree.workspaces.get(*ws_idx) {
                let arrow = if ws.collapsed { "▶ " } else { "▼ " };
                format!("{}{}", arrow, ws.name)
            } else {
                format!("WS {}", ws_idx)
            };
            (
                ExpandedItemPresentation {
                    indent: INDENT_WS,
                },
                label,
            )
        }
        SidebarItem::Column { ws_idx, col_idx } => {
            let label = tree.workspaces
                .get(*ws_idx)
                .and_then(|ws| ws.columns.get(*col_idx))
                .map(|col| {
                    let arrow = if col.collapsed { "▶ " } else { "▼ " };
                    format!("{}{}", arrow, col.name)
                })
                .unwrap_or_else(|| format!("Col {}", col_idx + 1));
            (
                ExpandedItemPresentation {
                    indent: INDENT_COL,
                },
                label,
            )
        }
        SidebarItem::Pane { pane_id } => (
            ExpandedItemPresentation {
                indent: INDENT_PANE,
            },
            expanded_pane_label(*pane_id, false, tree, candidates, focused_pane),
        ),
        SidebarItem::FloatingPane { pane_id, .. } => (
            ExpandedItemPresentation {
                indent: INDENT_PANE,
            },
            expanded_pane_label(*pane_id, true, tree, candidates, focused_pane),
        ),
    }
}

fn expanded_pane_label(
    pane_id: PaneId,
    is_floating: bool,
    tree: &SidebarTree,
    candidates: Option<&[(char, PaneId)]>,
    focused_pane: Option<PaneId>,
) -> String {
    let base = if is_floating {
        format!("~ {}", pane_name_short(pane_id, tree))
    } else {
        pane_name_short(pane_id, tree)
    };

    if let Some(ch) = candidate_char(pane_id, candidates, focused_pane) {
        format!("[{}] {}", ch, base)
    } else {
        base
    }
}

fn push_disclosure_hitbox(
    flat_item: &SidebarItem,
    button_hitboxes: &mut Vec<SidebarButtonHitbox>,
    text_x: f32,
    line_y: f32,
) {
    if flat_item.is_expandable() {
        button_hitboxes.push(SidebarButtonHitbox {
            action: WmAction::SidebarExpandToggle,
            ws_idx: None,
            x: text_x,
            y: line_y,
            width: 16.0,
            height: ITEM_HEIGHT,
        });
    }
}

fn expanded_item_text_color(
    flat_item: &SidebarItem,
    is_cursor: bool,
    tree: &SidebarTree,
    colors: RenderColors,
) -> [f32; 4] {
    if is_cursor {
        return colors.accent;
    }

    if flat_item.kind() == SidebarItemKind::Workspace {
        return flat_item
            .workspace_idx()
            .and_then(|wi| tree.workspaces.get(wi))
            .map(|ws| item_state_color(ws.state, colors.accent, colors.foreground, colors.visited_color))
            .unwrap_or(colors.foreground);
    }

    match flat_item {
        SidebarItem::Pane { pane_id } | SidebarItem::FloatingPane { pane_id, .. } => {
            tree.pane_entry(*pane_id)
                .map(|pane| {
                    item_state_color(pane.state, colors.accent, colors.foreground, colors.visited_color)
                })
                .unwrap_or(colors.foreground)
        }
        _ => colors.foreground,
    }
}

fn draw_expanded_pane_background(
    primitive_renderer: &mut PrimitiveRenderer,
    flat_item: &SidebarItem,
    x: f32,
    line_y: f32,
    width: f32,
    tree: &SidebarTree,
    colors: RenderColors,
) {
    if let SidebarItem::Pane { pane_id } | SidebarItem::FloatingPane { pane_id, .. } = flat_item
        && let Some(pane) = tree.pane_entry(*pane_id)
    {
        match pane.state {
            SidebarItemState::Active => {
                let mut bg = colors.accent;
                bg[3] = 0.18;
                primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, bg);
            }
            SidebarItemState::Visited => {
                let mut bg = colors.visited_color;
                bg[3] = 0.10;
                primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, bg);
            }
            SidebarItemState::None => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_expanded_item_buttons(
    flat_item: &SidebarItem,
    button_hitboxes: &mut Vec<SidebarButtonHitbox>,
    x: f32,
    width: f32,
    line_y: f32,
    foreground: [f32; 4],
    hovered_btn_idx: Option<usize>,
    button_font_size: f32,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
) {
    let btn_x_right = x + width - BTN_SIZE - BTN_PAD_X - 4.0;
    let btn_y = line_y + (ITEM_HEIGHT - BTN_SIZE) / 2.0;

    match flat_item {
        SidebarItem::Workspace { ws_idx } => {
            let add_x = btn_x_right - BTN_SIZE - BTN_PAD_X;
            let add_colors = button_colors(foreground, hovered_btn_idx == Some(button_hitboxes.len()));
            draw_sidebar_button(
                button_hitboxes,
                text_renderer,
                primitive_renderer,
                "+c",
                ButtonPlacement { x: add_x, y: btn_y },
                add_colors,
                button_font_size,
                WmAction::SplitHorizontal,
                Some(*ws_idx),
            );

            let delete_colors = delete_button_colors(hovered_btn_idx == Some(button_hitboxes.len()));
            draw_sidebar_button(
                button_hitboxes,
                text_renderer,
                primitive_renderer,
                "-",
                ButtonPlacement {
                    x: btn_x_right,
                    y: btn_y,
                },
                delete_colors,
                button_font_size,
                WmAction::DeleteWorkspace { ws_idx: *ws_idx },
                Some(*ws_idx),
            );
        }
        SidebarItem::Column { ws_idx, col_idx } => {
            let add_x = btn_x_right - BTN_SIZE - BTN_PAD_X;
            let add_colors = button_colors(foreground, hovered_btn_idx == Some(button_hitboxes.len()));
            draw_sidebar_button(
                button_hitboxes,
                text_renderer,
                primitive_renderer,
                "+p",
                ButtonPlacement { x: add_x, y: btn_y },
                add_colors,
                button_font_size,
                WmAction::AddPaneToColumn {
                    ws_idx: *ws_idx,
                    col_idx: *col_idx,
                },
                Some(*ws_idx),
            );

            let delete_colors = delete_button_colors(hovered_btn_idx == Some(button_hitboxes.len()));
            draw_sidebar_button(
                button_hitboxes,
                text_renderer,
                primitive_renderer,
                "-",
                ButtonPlacement {
                    x: btn_x_right,
                    y: btn_y,
                },
                delete_colors,
                button_font_size,
                WmAction::DeleteColumn {
                    ws_idx: *ws_idx,
                    col_idx: *col_idx,
                },
                Some(*ws_idx),
            );
        }
        SidebarItem::Pane { pane_id } => {
            let delete_colors = delete_button_colors(hovered_btn_idx == Some(button_hitboxes.len()));
            draw_sidebar_button(
                button_hitboxes,
                text_renderer,
                primitive_renderer,
                "-",
                ButtonPlacement {
                    x: btn_x_right,
                    y: btn_y,
                },
                delete_colors,
                button_font_size,
                WmAction::ClosePaneById { pane_id: *pane_id },
                None,
            );
        }
        SidebarItem::FloatingPane { .. } => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn render_create_workspace_button(
    button_hitboxes: &mut Vec<SidebarButtonHitbox>,
    x: f32,
    width: f32,
    line_y: f32,
    foreground: [f32; 4],
    hovered_btn_idx: Option<usize>,
    button_font_size: f32,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
) {
    let placement = ButtonPlacement {
        x: x + width - BTN_SIZE - BTN_PAD_X - 4.0,
        y: line_y + (ITEM_HEIGHT - BTN_SIZE) / 2.0,
    };
    let colors = button_colors(foreground, hovered_btn_idx == Some(button_hitboxes.len()));
    draw_sidebar_button(
        button_hitboxes,
        text_renderer,
        primitive_renderer,
        "+w",
        placement,
        colors,
        button_font_size,
        WmAction::CreateWorkspace,
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_sidebar_button(
    button_hitboxes: &mut Vec<SidebarButtonHitbox>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    label: &str,
    placement: ButtonPlacement,
    colors: ButtonColors,
    button_font_size: f32,
    action: WmAction,
    ws_idx: Option<usize>,
) {
    primitive_renderer.draw_rounded_rect(
        placement.x,
        placement.y,
        BTN_SIZE,
        BTN_SIZE,
        colors.bg,
        colors.border,
        1.0,
        BTN_RADIUS,
    );

    let text_width = label.chars().count() as f32 * button_font_size * 0.55;
    text_renderer.queue_text(
        label,
        placement.x + (BTN_SIZE - text_width) / 2.0,
        placement.y + (BTN_SIZE - button_font_size) / 2.0 + 2.0,
        button_font_size,
        colors.text,
    );

    button_hitboxes.push(SidebarButtonHitbox {
        action,
        ws_idx,
        x: placement.x,
        y: placement.y,
        width: BTN_SIZE,
        height: BTN_SIZE,
    });
}

#[allow(clippy::too_many_arguments)]
fn render_collapsed_columns(
    ws: &SidebarWsEntry,
    cursor: usize,
    flat_idx: &mut usize,
    line_y: &mut f32,
    x: f32,
    width: f32,
    text_x: f32,
    font_size: f32,
    is_sidebar_nav: bool,
    candidates: Option<&[(char, PaneId)]>,
    focused_pane: Option<PaneId>,
    colors: RenderColors,
    drag_hover: Option<DragItemId>,
    drag_source: Option<DragItemId>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
) {
    for col in &ws.columns {
        *flat_idx += 1;
        for pane in &col.panes {
            let item_idx = *flat_idx;
            let is_pane_cursor = is_sidebar_nav && item_idx == cursor;
            *flat_idx += 1;

            if is_pane_cursor {
                primitive_renderer.draw_rect(x, *line_y, width, ITEM_HEIGHT, colors.cursor_bg);
            }

            draw_drag_hover(
                primitive_renderer,
                x,
                *line_y,
                width,
                colors.accent,
                0.25,
                drag_hover == Some(DragItemId::new(item_idx)),
            );
            draw_drag_source(
                primitive_renderer,
                x,
                *line_y,
                width,
                colors.drag_source_bg,
                colors.drag_source_border,
                drag_source == Some(DragItemId::new(item_idx)),
            );

            let pane_char = collapsed_pane_label(pane, candidates, focused_pane, '?');
            let pane_color = if is_pane_cursor {
                colors.accent
            } else {
                item_state_color(pane.state, colors.accent, colors.foreground, colors.visited_color)
            };
            let pane_text_y = *line_y + (ITEM_HEIGHT - font_size) / 2.0;
            text_renderer.queue_text(&pane_char, text_x, pane_text_y, font_size, pane_color);
            *line_y += ITEM_HEIGHT;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_collapsed_floating_panes(
    ws: &SidebarWsEntry,
    flat_idx: &mut usize,
    line_y: &mut f32,
    x: f32,
    width: f32,
    text_x: f32,
    font_size: f32,
    candidates: Option<&[(char, PaneId)]>,
    focused_pane: Option<PaneId>,
    colors: RenderColors,
    drag_hover: Option<DragItemId>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
) {
    for pane in &ws.floating_panes {
        let item_idx = *flat_idx;
        *flat_idx += 1;

        draw_drag_hover(
            primitive_renderer,
            x,
            *line_y,
            width,
            colors.accent,
            0.18,
            drag_hover == Some(DragItemId::new(item_idx)),
        );

        let pane_char = collapsed_pane_label(pane, candidates, focused_pane, '~');
        let pane_color = item_state_color(
            pane.state,
            colors.accent,
            colors.foreground,
            colors.visited_color,
        );
        let pane_text_y = *line_y + (ITEM_HEIGHT - font_size) / 2.0;
        text_renderer.queue_text(&pane_char, text_x, pane_text_y, font_size, pane_color);
        *line_y += ITEM_HEIGHT;
    }
}

fn draw_activity_bar(
    primitive_renderer: &mut PrimitiveRenderer,
    x: f32,
    line_y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
) {
    if color[3] > 0.0 {
        primitive_renderer.draw_rect(x, line_y, width, height, color);
    }
}

fn draw_drag_hover(
    primitive_renderer: &mut PrimitiveRenderer,
    x: f32,
    line_y: f32,
    width: f32,
    accent: [f32; 4],
    alpha: f32,
    enabled: bool,
) {
    if enabled {
        let mut drag_bg = accent;
        drag_bg[3] = alpha;
        primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_bg);
    }
}

fn draw_drag_source(
    primitive_renderer: &mut PrimitiveRenderer,
    x: f32,
    line_y: f32,
    width: f32,
    bg: [f32; 4],
    border: [f32; 4],
    enabled: bool,
) {
    if enabled {
        primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, bg);
        primitive_renderer.draw_border(
            x + 1.0,
            line_y + 1.0,
            width - 2.0,
            ITEM_HEIGHT - 2.0,
            border,
            1.0,
        );
    }
}

fn candidate_char(
    pane_id: PaneId,
    candidates: Option<&[(char, PaneId)]>,
    focused_pane: Option<PaneId>,
) -> Option<char> {
    candidates.and_then(|cands| {
        if focused_pane == Some(pane_id) {
            None
        } else {
            cands.iter().find(|(_, pid)| *pid == pane_id).map(|(ch, _)| *ch)
        }
    })
}

fn collapsed_pane_label(
    pane: &SidebarPaneEntry,
    candidates: Option<&[(char, PaneId)]>,
    focused_pane: Option<PaneId>,
    fallback: char,
) -> String {
    candidate_char(pane.pane_id, candidates, focused_pane)
        .map(|ch| ch.to_string())
        .unwrap_or_else(|| pane.name.chars().next().unwrap_or(fallback).to_string())
}

fn item_state_color(
    state: SidebarItemState,
    accent: [f32; 4],
    foreground: [f32; 4],
    visited_color: [f32; 4],
) -> [f32; 4] {
    match state {
        SidebarItemState::Active => accent,
        SidebarItemState::Visited => visited_color,
        SidebarItemState::None => foreground,
    }
}

fn button_colors(foreground: [f32; 4], is_hovered: bool) -> ButtonColors {
    if is_hovered {
        ButtonColors {
            bg: [foreground[0], foreground[1], foreground[2], 0.3],
            border: [foreground[0], foreground[1], foreground[2], 0.7],
            text: foreground,
        }
    } else {
        ButtonColors {
            bg: [foreground[0], foreground[1], foreground[2], 0.12],
            border: [foreground[0], foreground[1], foreground[2], 0.35],
            text: foreground,
        }
    }
}

fn delete_button_colors(is_hovered: bool) -> ButtonColors {
    if is_hovered {
        ButtonColors {
            bg: [0.9, 0.3, 0.3, 0.4],
            border: [0.9, 0.3, 0.3, 0.8],
            text: [0.95, 0.4, 0.4, 1.0],
        }
    } else {
        ButtonColors {
            bg: [0.9, 0.3, 0.3, 0.15],
            border: [0.9, 0.3, 0.3, 0.4],
            text: [0.9, 0.3, 0.3, 0.9],
        }
    }
}

fn pane_name_short(pane_id: PaneId, tree: &SidebarTree) -> String {
    tree.pane_entry(pane_id)
        .map(|pane| pane.name.clone())
        .unwrap_or_else(|| format!("Pane {}", pane_id))
}
