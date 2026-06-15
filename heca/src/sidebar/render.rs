use crate::app_state::SidebarItemState;
use heca_grid_ui::drag::DragItemId;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_core::layout::PaneId;
use heca_renderer::text::TextRenderer;

use super::{SidebarPaneEntry, SidebarTree, SidebarWsEntry};

pub(crate) const ITEM_HEIGHT: f32 = 24.0;

#[derive(Clone, Copy)]
struct RenderColors {
    accent: [f32; 4],
    foreground: [f32; 4],
    cursor_bg: [f32; 4],
    visited_color: [f32; 4],
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
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

