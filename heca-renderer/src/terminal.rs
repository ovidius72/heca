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

}

fn render_terminal_lines(
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    lines: &[TerminalLine],
    cursor: heca_core::backend::TerminalCursor,
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

        let mut run_text = String::new();
        let mut run_col = 0usize;
        let mut run_cells = 0usize;
        let mut run_fg = [0.0; 4];
        let mut run_bold = false;
        let mut run_italic = false;
        let mut run_underline = false;

        let flush_run = |text_renderer: &mut TextRenderer,
                         primitive_renderer: &mut PrimitiveRenderer,
                         run_text: &mut String,
                         run_col: usize,
                         run_cells: &mut usize,
                         run_fg: [f32; 4],
                         run_bold: bool,
                         run_italic: bool,
                         run_underline: bool| {
            if run_text.is_empty() {
                return;
            }
            let x = px + run_col as f32 * cell_w;
            text_renderer.queue_text_in_line_box_with_style(
                run_text,
                TextBox {
                    x,
                    y,
                    w: (*run_cells as f32) * cell_w,
                    h: cell_h,
                },
                style.font_size,
                TextStyle {
                    color: run_fg,
                    bold: run_bold,
                    italic: run_italic && style.italic_font_family != style.font_family,
                    faux_italic: run_italic && style.italic_font_family == style.font_family,
                    font_family: Some(if run_italic { style.italic_font_family } else { style.font_family }),
                },
                TextAlign::Start,
                false,
            );
            if run_underline {
                let underline_h = (cell_h * 0.08).max(1.0);
                let underline_y = y + cell_h - underline_h - (cell_h * 0.08).max(1.0);
                primitive_renderer.draw_rect(
                    x,
                    underline_y,
                    (*run_cells as f32) * cell_w,
                    underline_h,
                    run_fg,
                );
            }
            run_text.clear();
            *run_cells = 0;
        };

        for (col, cell) in line.cells.iter().take(visible_cols).enumerate() {
            let is_blank = cell.text.trim().is_empty();
            let single_width = cell.width == 1;
            let x = px + col as f32 * cell_w;

            if is_blank || !single_width {
                flush_run(
                    text_renderer,
                    primitive_renderer,
                    &mut run_text,
                    run_col,
                    &mut run_cells,
                    run_fg,
                    run_bold,
                    run_italic,
                    run_underline,
                );
                if !is_blank {
                    if !draw_box_drawing_cell(
                        primitive_renderer,
                        &cell.text,
                        x,
                        y,
                        (cell.width.max(1) as f32) * cell_w,
                        cell_h,
                        cell.fg,
                    ) {
                        text_renderer.queue_text_in_line_box_with_style(
                            &cell.text,
                            TextBox {
                                x,
                                y,
                                w: (cell.width.max(1) as f32) * cell_w,
                                h: cell_h,
                            },
                            style.font_size,
                            TextStyle {
                                color: cell.fg,
                                bold: cell.bold,
                                italic: cell.italic && style.italic_font_family != style.font_family,
                                faux_italic: cell.italic && style.italic_font_family == style.font_family,
                                font_family: Some(if cell.italic { style.italic_font_family } else { style.font_family }),
                            },
                            TextAlign::Start,
                            false,
                        );
                    }
                    if cell.underline {
                        let underline_h = (cell_h * 0.08).max(1.0);
                        let underline_y = y + cell_h - underline_h - (cell_h * 0.08).max(1.0);
                        primitive_renderer.draw_rect(
                            x,
                            underline_y,
                            (cell.width.max(1) as f32) * cell_w,
                            underline_h,
                            cell.fg,
                        );
                    }
                }
                continue;
            }

            if run_text.is_empty() {
                run_col = col;
                run_fg = cell.fg;
                run_bold = cell.bold;
                run_italic = cell.italic;
                run_underline = cell.underline;
            } else if cell.fg != run_fg
                || cell.bold != run_bold
                || cell.italic != run_italic
                || cell.underline != run_underline
            {
                flush_run(
                    text_renderer,
                    primitive_renderer,
                    &mut run_text,
                    run_col,
                    &mut run_cells,
                    run_fg,
                    run_bold,
                    run_italic,
                    run_underline,
                );
                run_col = col;
                run_fg = cell.fg;
                run_bold = cell.bold;
                run_italic = cell.italic;
                run_underline = cell.underline;
            }

            if draw_box_drawing_cell(
                primitive_renderer,
                &cell.text,
                x,
                y,
                cell_w,
                cell_h,
                cell.fg,
            ) {
                flush_run(
                    text_renderer,
                    primitive_renderer,
                    &mut run_text,
                    run_col,
                    &mut run_cells,
                    run_fg,
                    run_bold,
                    run_italic,
                    run_underline,
                );
                continue;
            }

            run_text.push_str(&cell.text);
            run_cells += cell.width.max(1);
        }

        flush_run(
            text_renderer,
            primitive_renderer,
            &mut run_text,
            run_col,
            &mut run_cells,
            run_fg,
            run_bold,
            run_italic,
            run_underline,
        );
    }

    if cursor.visible
        && cursor.row < max_rows
        && cursor.col
            < lines
                .get(cursor.row)
                .map(|line| max_cols.min(line.cells.len()))
                .unwrap_or(0)
    {
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
                let bar_w = (cell_w * 0.12).max(2.0);
                primitive_renderer.draw_rect(cursor_x, cursor_y, bar_w, cell_h, cursor_color);
            }
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
