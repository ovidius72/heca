use crate::primitive::PrimitiveRenderer;
use crate::text::{TextBox, TextRenderer, TextStyle};
use heca_core::backend::{
    TerminalCursorShape, TerminalLine, TerminalSnapshot, TerminalUnderlineStyle,
};
use heca_grid_ui::scene::TextAlign;

/// Host-level selection overlay parameters for terminal panes.
///
/// The renderer is agnostic of the shared selection model; it receives
/// pre-computed row spans and a color from the app layer. The app layer is
/// responsible for converting anchor/focus cell coordinates into row-wise
/// spans that follow terminal text-flow semantics.
#[derive(Clone, Copy, Debug)]
pub struct SelectionOverlaySpan {
    pub row: usize,
    pub start_col: usize,
    pub end_col: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct CaretIndicator {
    pub row: usize,
    pub col: usize,
    /// When true, the caret marks the active endpoint of an existing
    /// selection (thicker, more prominent). When false, the caret is in
    /// caret-only state (thin bar, no selection yet).
    pub is_selection_endpoint: bool,
}

#[derive(Clone, Debug)]
pub struct SelectionOverlay {
    pub spans: Vec<SelectionOverlaySpan>,
    pub color: [f32; 4],
    /// Caret indicator: drawn as a blinking cursor-like block when in
    /// caret-only state (selection mode entered but no selection started).
    pub caret: Option<CaretIndicator>,
}

impl SelectionOverlay {
    pub fn new(spans: Vec<SelectionOverlaySpan>, color: [f32; 4]) -> Self {
        Self { spans, color, caret: None }
    }

    pub fn with_caret(mut self, caret: CaretIndicator) -> Self {
        self.caret = Some(caret);
        self
    }
}

pub struct TerminalStyle<'a> {
    pub font_size: f32,
    pub font_family: &'a str,
    pub italic_font_family: &'a str,
    /// Alpha multiplier for the terminal surface background.
    /// 1.0 = fully opaque (no transparency). When < 1.0, the default-bg fill
    /// becomes translucent so a frosted backdrop can show through.
    /// Derived from `AppearanceConfig::opacity()`.
    pub surface_alpha: f32,
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
        queue_cursor_overlay(self.primitive_renderer, snapshot, rect);
    }

    /// Draw a host-level selection overlay inside the terminal content rect.
    ///
    /// Intended to be called after cell backgrounds and glyphs (so the
    /// selected text remains readable) and before the cursor overlay (so the
    /// cursor is always visible on top). The caller (`render_terminal_mount`)
    /// is responsible for enforcing this ordering.
    ///
    /// Each span is clipped to the fitted grid and rendered as its own
    /// rectangle, so multi-line selections follow row semantics rather than
    /// painting one large bounding box.
    pub fn render_selection_overlay(
        &mut self,
        overlay: &SelectionOverlay,
        rect: TextBox,
        cell_w: f32,
        cell_h: f32,
    ) {
        let (fitted_rows, fitted_cols) = fitted_grid(rect, cell_w, cell_h);
        if fitted_rows == 0 || fitted_cols == 0 {
            return;
        }

        for span in &overlay.spans {
            let visible_row = span.row.min(fitted_rows.saturating_sub(1));
            let visible_start_col = span.start_col.min(fitted_cols.saturating_sub(1));
            let visible_end_col = span.end_col.min(fitted_cols.saturating_sub(1));
            if visible_start_col > visible_end_col {
                continue;
            }

            let y = rect.y + visible_row as f32 * cell_h;
            let x = rect.x + visible_start_col as f32 * cell_w;
            let w = (visible_end_col - visible_start_col + 1) as f32 * cell_w;
            self.primitive_renderer
                .draw_rect(x, y, w, cell_h, overlay.color);
        }

        // Draw caret indicator.
        //
        // Two visual modes:
        // - Caret-only (no selection): thin 2px bar at 0.8 alpha — the user
        //   hasn't started selecting yet, so the caret is subtle but visible.
        // - Selection endpoint: wider 4px bar at 0.9 alpha — the user is
        //   actively growing a selection and needs to see exactly which end
        //   will move when they press h/j/k/l or after toggling with `o`.
        if let Some(caret) = &overlay.caret {
            let visible_row = caret.row.min(fitted_rows.saturating_sub(1));
            let visible_col = caret.col.min(fitted_cols.saturating_sub(1));
            let y = rect.y + visible_row as f32 * cell_h;
            let (caret_w, caret_alpha, x) = if caret.is_selection_endpoint {
                // Selection endpoint: draw at the RIGHT boundary of the focus
                // cell so it visually marks the end of the selected range.
                // 4px wide, 0.9 alpha, right-aligned to the cell edge.
                let w = 4.0;
                let x = rect.x + (visible_col + 1) as f32 * cell_w - w;
                (w, 0.9, x)
            } else {
                // Caret-only (no selection): thin 2px bar at the LEFT edge of
                // the cell, like a normal text cursor.
                let w = 2.0;
                let x = rect.x + visible_col as f32 * cell_w;
                (w, 0.8, x)
            };
            let caret_color = [
                overlay.color[0],
                overlay.color[1],
                overlay.color[2],
                caret_alpha,
            ];
            self.primitive_renderer
                .draw_rect(x, y, caret_w, cell_h, caret_color);
        }
    }

}

fn render_terminal_lines(
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    lines: &[TerminalLine],
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
        ..
    } = rect;
    let (fitted_rows, fitted_cols) = fitted_grid(rect, cell_w, cell_h);
    if fitted_rows == 0 || fitted_cols == 0 {
        return;
    }
    // The terminal owns the translucent surface inside the pane content rect.
    // The outer pane shell owns border/radius/highlight only.
    let surface_bg = [
        default_bg[0],
        default_bg[1],
        default_bg[2],
        default_bg[3] * style.surface_alpha,
    ];
    if surface_bg[3] > 0.0 {
        primitive_renderer.draw_rect(px, py, pw, ph, surface_bg);
    }
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
        while bg_start < visible_cols {
            let bg_color = line.cells[bg_start].bg;
            let mut bg_end = bg_start + 1;
            while bg_end < visible_cols && line.cells[bg_end].bg == bg_color {
                bg_end += 1;
            }
            if !is_default_bg(bg_color, default_bg) {
                let bg_color = with_surface_alpha(bg_color, style.surface_alpha);
                if bg_color[3] > 0.0 {
                    let x = px + bg_start as f32 * cell_w;
                    let w = (bg_end - bg_start) as f32 * cell_w;
                    primitive_renderer.draw_rect(x, y, w, cell_h, bg_color);
                }
            }
            bg_start = bg_end;
        }

        for (col, cell) in line.cells.iter().take(visible_cols).enumerate() {
            let is_blank = cell.text.trim().is_empty();
            let x = px + col as f32 * cell_w;
            let width = (cell.width.max(1) as f32) * cell_w;

            if is_blank {
                continue;
            }

            if draw_terminal_symbol_cell(
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
            draw_underline_style(
                primitive_renderer,
                cell.underline,
                x,
                y,
                width,
                cell_h,
                cell.fg,
            );
        }
    }
}

fn with_surface_alpha(color: [f32; 4], surface_alpha: f32) -> [f32; 4] {
    [color[0], color[1], color[2], color[3] * surface_alpha.clamp(0.0, 1.0)]
}

fn is_default_bg(bg: [f32; 4], default_bg: [f32; 4]) -> bool {
    const EPS: f32 = 1e-6;
    (bg[0] - default_bg[0]).abs() < EPS
        && (bg[1] - default_bg[1]).abs() < EPS
        && (bg[2] - default_bg[2]).abs() < EPS
        && (bg[3] - default_bg[3]).abs() < EPS
}

fn queue_cursor_overlay(
    primitive_renderer: &mut PrimitiveRenderer,
    snapshot: &TerminalSnapshot,
    rect: TextBox,
) {
    let cursor = snapshot.cursor;
    let lines = snapshot.lines.as_slice();
    let cols = snapshot.cols;
    let rows = snapshot.rows;
    let cell_w = snapshot.cell_w;
    let cell_h = snapshot.cell_h;
    let TextBox { x: px, y: py, .. } = rect;
    let (fitted_rows, fitted_cols) = fitted_grid(rect, cell_w, cell_h);
    if fitted_rows == 0 || fitted_cols == 0 {
        return;
    }
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

fn draw_underline_style(
    primitive_renderer: &mut PrimitiveRenderer,
    style: TerminalUnderlineStyle,
    x: f32,
    y: f32,
    width: f32,
    cell_h: f32,
    color: [f32; 4],
) {
    if !style.is_visible() {
        return;
    }

    let stroke = (cell_h * 0.08).max(1.0);
    let baseline_gap = (cell_h * 0.08).max(1.0);
    let baseline_y = y + cell_h - stroke - baseline_gap;

    match style {
        TerminalUnderlineStyle::None => {}
        TerminalUnderlineStyle::Single => {
            primitive_renderer.draw_rect(x, baseline_y, width, stroke, color);
        }
        TerminalUnderlineStyle::Double => {
            let separation = (stroke * 1.6).max(2.0);
            primitive_renderer.draw_rect(x, baseline_y, width, stroke, color);
            primitive_renderer.draw_rect(
                x,
                (baseline_y - separation).max(y),
                width,
                stroke,
                color,
            );
        }
        TerminalUnderlineStyle::Dotted => {
            let dot = stroke.max(1.0);
            let gap = dot.max(1.0);
            let mut cursor = x;
            while cursor < x + width {
                let span = dot.min(x + width - cursor);
                primitive_renderer.draw_rect(cursor, baseline_y, span, stroke, color);
                cursor += dot + gap;
            }
        }
        TerminalUnderlineStyle::Dashed => {
            let dash = (cell_h * 0.35).max(stroke * 2.0);
            let gap = (dash * 0.45).max(1.0);
            let mut cursor = x;
            while cursor < x + width {
                let span = dash.min(x + width - cursor);
                primitive_renderer.draw_rect(cursor, baseline_y, span, stroke, color);
                cursor += dash + gap;
            }
        }
        TerminalUnderlineStyle::Curly => {
            draw_curly_underline(
                primitive_renderer,
                TextBox {
                    x,
                    y,
                    w: width,
                    h: cell_h,
                },
                baseline_y,
                stroke,
                color,
            );
        }
    }
}

fn draw_curly_underline(
    primitive_renderer: &mut PrimitiveRenderer,
    rect: TextBox,
    y: f32,
    stroke: f32,
    color: [f32; 4],
) {
    let period = (stroke * 4.0).max(6.0);
    let amp = ((stroke * 1.4).max(1.5))
        .min((y - rect.y).max(0.0))
        .min((rect.y + rect.h - stroke - y).max(0.0));
    if amp <= 0.0 {
        primitive_renderer.draw_rect(rect.x, y, rect.w, stroke, color);
        return;
    }
    let step = (stroke * 0.9).max(1.0);
    let mut cursor = rect.x;

    while cursor < rect.x + rect.w {
        let next = (cursor + step).min(rect.x + rect.w);
        let mid = (cursor + next) * 0.5;
        let phase = ((mid - rect.x) / period) * std::f32::consts::TAU;
        let center_y = y + phase.sin() * amp;
        primitive_renderer.draw_rect(cursor, center_y, (next - cursor).max(1.0), stroke, color);
        cursor = next;
    }
}

fn draw_terminal_symbol_cell(
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

    if draw_powerline_cell(primitive_renderer, ch, x, y, w, h, color) {
        return true;
    }

    draw_box_drawing_cell(primitive_renderer, ch, x, y, w, h, color)
}

#[cfg(test)]
mod tests {
    use super::{is_default_bg, with_surface_alpha};

    #[test]
    fn with_surface_alpha_scales_alpha_only() {
        let color = [0.25, 0.5, 0.75, 0.8];
        let scaled = with_surface_alpha(color, 0.5);
        assert_eq!(scaled[0], color[0]);
        assert_eq!(scaled[1], color[1]);
        assert_eq!(scaled[2], color[2]);
        assert!((scaled[3] - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn with_surface_alpha_clamps_input() {
        let color = [1.0, 1.0, 1.0, 0.8];
        assert!((with_surface_alpha(color, 2.0)[3] - 0.8).abs() < f32::EPSILON);
        assert!((with_surface_alpha(color, -1.0)[3] - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn is_default_bg_uses_exact_visual_match() {
        let bg = [0.1, 0.2, 0.3, 1.0];
        assert!(is_default_bg(bg, bg));
        assert!(!is_default_bg(bg, [0.1, 0.2, 0.31, 1.0]));
    }
}

fn draw_box_drawing_cell(
    primitive_renderer: &mut PrimitiveRenderer,
    ch: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
) -> bool {
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

fn draw_powerline_cell(
    primitive_renderer: &mut PrimitiveRenderer,
    ch: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
) -> bool {
    let stroke = (w.min(h) * 0.08).max(1.0);

    match ch {
        '' => {
            primitive_renderer.draw_triangle([x, y], [x, y + h], [x + w, y + h * 0.5], color);
        }
        '' => {
            primitive_renderer.draw_triangle(
                [x + w, y],
                [x + w, y + h],
                [x, y + h * 0.5],
                color,
            );
        }
        '' => {
            draw_segmented_line(
                primitive_renderer,
                [x, y],
                [x + w, y + h * 0.5],
                stroke,
                color,
            );
            draw_segmented_line(
                primitive_renderer,
                [x + w, y + h * 0.5],
                [x, y + h],
                stroke,
                color,
            );
        }
        '' => {
            draw_segmented_line(
                primitive_renderer,
                [x + w, y],
                [x, y + h * 0.5],
                stroke,
                color,
            );
            draw_segmented_line(
                primitive_renderer,
                [x, y + h * 0.5],
                [x + w, y + h],
                stroke,
                color,
            );
        }
        '' => {
            draw_half_ellipse(primitive_renderer, x, y, w, h, true, color);
        }
        '' => {
            draw_half_ellipse(primitive_renderer, x, y, w, h, false, color);
        }
        _ => return false,
    }

    true
}

fn draw_segmented_line(
    primitive_renderer: &mut PrimitiveRenderer,
    start: [f32; 2],
    end: [f32; 2],
    stroke: f32,
    color: [f32; 4],
) {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx.abs().max(dy.abs());
    let steps = ((length / stroke.max(1.0)).ceil() as usize).max(1);
    let dot = stroke.max(1.0);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let px = start[0] + dx * t - dot * 0.5;
        let py = start[1] + dy * t - dot * 0.5;
        primitive_renderer.draw_rect(px, py, dot, dot, color);
    }
}

fn draw_half_ellipse(
    primitive_renderer: &mut PrimitiveRenderer,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bulge_right: bool,
    color: [f32; 4],
) {
    let bands = h.ceil().clamp(8.0, 32.0) as usize;
    let band_h = h / bands as f32;
    for i in 0..bands {
        let band_y = y + band_h * i as f32;
        let next_band_y = y + band_h * (i + 1) as f32;
        let draw_h = (next_band_y - band_y).max(0.0);
        let t = ((i as f32 + 0.5) / bands as f32) * 2.0 - 1.0;
        let radius = (1.0 - t * t).max(0.0).sqrt();
        let inset = w * (1.0 - radius);
        if bulge_right {
            primitive_renderer.draw_rect(x, band_y, (w - inset).max(0.0), draw_h, color);
        } else {
            primitive_renderer.draw_rect(
                x + inset,
                band_y,
                (w - inset).max(0.0),
                draw_h,
                color,
            );
        }
    }
}

fn fitted_grid(rect: TextBox, cell_w: f32, cell_h: f32) -> (usize, usize) {
    if !rect.w.is_finite()
        || !rect.h.is_finite()
        || !cell_w.is_finite()
        || !cell_h.is_finite()
        || rect.w <= 0.0
        || rect.h <= 0.0
        || cell_w <= 0.0
        || cell_h <= 0.0
    {
        return (0, 0);
    }

    let rows = (rect.h / cell_h).round();
    let cols = (rect.w / cell_w).round();
    if !rows.is_finite() || !cols.is_finite() || rows <= 0.0 || cols <= 0.0 {
        return (0, 0);
    }

    (
        rows.min(usize::MAX as f32) as usize,
        cols.min(usize::MAX as f32) as usize,
    )
}
