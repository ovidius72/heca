use crate::primitive::PrimitiveRenderer;
use crate::text::{TextBox, TextRenderer, TextStyle};
use heca_core::backend::{TerminalCursorShape, TerminalLine, TerminalSnapshot};
use heca_grid_ui::scene::TextAlign;

pub struct TerminalStyle<'a> {
    pub font_size: f32,
    pub font_family: &'a str,
    pub italic_font_family: &'a str,
}

pub struct TerminalRenderer<'a> {
    text_renderer: &'a mut TextRenderer,
    primitive_renderer: &'a mut PrimitiveRenderer,
}

struct TerminalRenderLayout<'a> {
    cell_w: f32,
    cell_h: f32,
    cols: usize,
    rows: usize,
    style: TerminalStyle<'a>,
    rect: TextBox,
}

impl<'a> TerminalRenderer<'a> {
    pub fn new(
        text_renderer: &'a mut TextRenderer,
        primitive_renderer: &'a mut PrimitiveRenderer,
    ) -> Self {
        Self {
            text_renderer,
            primitive_renderer,
        }
    }

    pub fn render_snapshot(
        &mut self,
        snapshot: &TerminalSnapshot,
        rect: TextBox,
        style: TerminalStyle<'_>,
    ) {
        render_terminal_lines(
            self.text_renderer,
            self.primitive_renderer,
            &snapshot.lines,
            snapshot.cursor,
            snapshot.default_bg,
            TerminalRenderLayout {
                cell_w: snapshot.cell_w,
                cell_h: snapshot.cell_h,
                cols: snapshot.cols,
                rows: snapshot.rows,
                style,
                rect,
            },
        );
    }

    pub fn render_cursor_overlay(&mut self, snapshot: &TerminalSnapshot, rect: TextBox) {
        queue_cursor_overlay(
            self.primitive_renderer,
            snapshot.cursor,
            snapshot.lines.as_slice(),
            snapshot.cols,
            snapshot.rows,
            snapshot.cell_w,
            snapshot.cell_h,
            rect,
        );
    }

}

fn render_terminal_lines(
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    lines: &[TerminalLine],
    _cursor: heca_core::backend::TerminalCursor,
    default_bg: [f32; 4],
    layout: TerminalRenderLayout<'_>,
) {
    let TerminalRenderLayout {
        cell_w,
        cell_h,
        cols,
        rows,
        style,
        rect,
    } = layout;
    let TextBox {
        x: px,
        y: py,
        w: pw,
        h: ph,
    } = rect;
    primitive_renderer.draw_rect(px, py, pw, ph, default_bg);

    let fitted_rows = ((ph / cell_h).round() as usize).max(1);
    let fitted_cols = ((pw / cell_w).round() as usize).max(1);
    let max_rows = fitted_rows.min(rows).min(lines.len());
    let max_cols = fitted_cols.min(cols);
    if max_rows == 0 || max_cols == 0 {
        return;
    }

    for (row, line) in lines.iter().take(max_rows).enumerate() {
        let y = py + row as f32 * cell_h;
        let visible_cols = max_cols.min(line.cells.len());
        if visible_cols == 0 {
            continue;
        }
        let mut bg_start = 0usize;
        let mut bg_color = line
            .cells
            .first()
            .map(|cell| cell.bg)
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);

        for col in 0..visible_cols {
            let next_bg = if col + 1 < visible_cols {
                line.cells.get(col + 1).map(|next| next.bg)
            } else {
                None
            };
            if next_bg != Some(bg_color) {
                if bg_color[3] > 0.0 {
                    let x = px + bg_start as f32 * cell_w;
                    let w = (col + 1 - bg_start) as f32 * cell_w;
                    primitive_renderer.draw_rect(x, y, w, cell_h, bg_color);
                }
                bg_start = col + 1;
                if let Some(color) = next_bg {
                    bg_color = color;
                }
            }
        }

        for (col, cell) in line.cells.iter().take(visible_cols).enumerate() {
            let is_blank = cell.text.trim().is_empty();
            let x = px + col as f32 * cell_w;
            let width = (cell.width.max(1) as f32) * cell_w;

            if is_blank {
                continue;
            }

            if draw_box_drawing_cell(
                primitive_renderer,
                &cell.text,
                x,
                y,
                width,
                cell_h,
                cell.fg,
            ) {
                continue;
            }

            text_renderer.queue_text_in_line_box_with_style(
                &cell.text,
                TextBox {
                    x,
                    y,
                    w: width,
                    h: cell_h,
                },
                style.font_size,
                TextStyle {
                    color: cell.fg,
                    bold: cell.bold,
                    italic: cell.italic && style.italic_font_family != style.font_family,
                    faux_italic: cell.italic && style.italic_font_family == style.font_family,
                    font_family: Some(if cell.italic {
                        style.italic_font_family
                    } else {
                        style.font_family
                    }),
                },
                TextAlign::Start,
                false,
            );
            if cell.underline {
                let underline_h = (cell_h * 0.08).max(1.0);
                let underline_y = y + cell_h - underline_h - (cell_h * 0.08).max(1.0);
                primitive_renderer.draw_rect(x, underline_y, width, underline_h, cell.fg);
            }
        }
    }
}

fn queue_cursor_overlay(
    primitive_renderer: &mut PrimitiveRenderer,
    cursor: heca_core::backend::TerminalCursor,
    lines: &[TerminalLine],
    cols: usize,
    rows: usize,
    cell_w: f32,
    cell_h: f32,
    rect: TextBox,
) {
    let TextBox {
        x: px,
        y: py,
        w: pw,
        h: ph,
    } = rect;
    let fitted_rows = ((ph / cell_h).round() as usize).max(1);
    let fitted_cols = ((pw / cell_w).round() as usize).max(1);
    let max_rows = fitted_rows.min(rows).min(lines.len());
    let max_cols = fitted_cols.min(cols);

    if !cursor.visible
        || cursor.row >= max_rows
        || cursor.col
            >= lines
                .get(cursor.row)
                .map(|line| max_cols.min(line.cells.len()))
                .unwrap_or(0)
    {
        return;
    }

    let cursor_x = px + cursor.col as f32 * cell_w;
    let cursor_y = py + cursor.row as f32 * cell_h;
    let cursor_color = [1.0, 1.0, 1.0, 0.85];
    match cursor.shape {
        TerminalCursorShape::Block => {
            primitive_renderer.draw_rect(cursor_x, cursor_y, cell_w, cell_h, cursor_color);
        }
        TerminalCursorShape::Underline => {
            let underline_h = (cell_h * 0.12).max(2.0);
            primitive_renderer.draw_rect(
                cursor_x,
                cursor_y + cell_h - underline_h,
                cell_w,
                underline_h,
                cursor_color,
            );
        }
        TerminalCursorShape::Default | TerminalCursorShape::Bar => {
            let bar_w = (cell_w * 0.08).max(1.0);
            primitive_renderer.draw_rect(cursor_x, cursor_y, bar_w, cell_h, cursor_color);
        }
    }
}

fn draw_box_drawing_cell(
    primitive_renderer: &mut PrimitiveRenderer,
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
) -> bool {
    let Some(ch) = text.chars().next() else {
        return false;
    };
    if text.chars().nth(1).is_some() {
        return false;
    }

    let stroke = (w.min(h) * 0.08).max(1.5);
    let mid_x = x + w * 0.5 - stroke * 0.5;
    let mid_y = y + h * 0.5 - stroke * 0.5;

    match ch {
        '─' | '━' | '╌' | '╍' => {
            primitive_renderer.draw_rect(x, mid_y, w, stroke, color);
        }
        '│' | '┃' | '╎' | '╏' => {
            primitive_renderer.draw_rect(mid_x, y, stroke, h, color);
        }
        '┌' | '╭' => {
            primitive_renderer.draw_rect(mid_x, mid_y, w * 0.5 + stroke * 0.5, stroke, color);
            primitive_renderer.draw_rect(mid_x, mid_y, stroke, h * 0.5 + stroke * 0.5, color);
        }
        '┐' | '╮' => {
            primitive_renderer.draw_rect(x, mid_y, w * 0.5 + stroke * 0.5, stroke, color);
            primitive_renderer.draw_rect(mid_x, mid_y, stroke, h * 0.5 + stroke * 0.5, color);
        }
        '└' | '╰' => {
            primitive_renderer.draw_rect(mid_x, mid_y, w * 0.5 + stroke * 0.5, stroke, color);
            primitive_renderer.draw_rect(mid_x, y, stroke, h * 0.5 + stroke * 0.5, color);
        }
        '┘' | '╯' => {
            primitive_renderer.draw_rect(x, mid_y, w * 0.5 + stroke * 0.5, stroke, color);
            primitive_renderer.draw_rect(mid_x, y, stroke, h * 0.5 + stroke * 0.5, color);
        }
        '├' | '┝' | '┠' | '┣' => {
            primitive_renderer.draw_rect(mid_x, mid_y, w * 0.5 + stroke * 0.5, stroke, color);
            primitive_renderer.draw_rect(mid_x, y, stroke, h, color);
        }
        '┤' | '┥' | '┨' | '┫' => {
            primitive_renderer.draw_rect(x, mid_y, w * 0.5 + stroke * 0.5, stroke, color);
            primitive_renderer.draw_rect(mid_x, y, stroke, h, color);
        }
        '┬' | '┯' | '┰' | '┳' => {
            primitive_renderer.draw_rect(x, mid_y, w, stroke, color);
            primitive_renderer.draw_rect(mid_x, mid_y, stroke, h * 0.5 + stroke * 0.5, color);
        }
        '┴' | '┷' | '┸' | '┻' => {
            primitive_renderer.draw_rect(x, mid_y, w, stroke, color);
            primitive_renderer.draw_rect(mid_x, y, stroke, h * 0.5 + stroke * 0.5, color);
        }
        '┼' | '┿' | '╂' | '╋' | '╪' | '╫' => {
            primitive_renderer.draw_rect(x, mid_y, w, stroke, color);
            primitive_renderer.draw_rect(mid_x, y, stroke, h, color);
        }
        _ => return false,
    }

    true
}
