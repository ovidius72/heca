use super::*;
use super::decorations::{draw_hyperlink_decoration, draw_terminal_symbol_cell, draw_underline_style};
use crate::primitive::PrimitiveRenderer;
use crate::text::{TextBox, TextRenderer, TextStyle};
use heca_core::backend::{
    HyperlinkSpan, TerminalCursorShape, TerminalDamage, TerminalLine, TerminalRowRange,
    TerminalSnapshot,
};

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
    /// OSC 8 hyperlink spans over the visible grid (recolor + decorate).
    hyperlinks: &'a [HyperlinkSpan],
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
                hyperlinks: &snapshot.hyperlinks,
            },
            None,
        );
    }

    pub fn render_snapshot_damage(
        &mut self,
        snapshot: &TerminalSnapshot,
        rect: TextBox,
        style: TerminalStyle<'_>,
        damage: &TerminalDamage,
    ) {
        match damage {
            TerminalDamage::None => {}
            TerminalDamage::Full => self.render_snapshot(snapshot, rect, style),
            TerminalDamage::Rows(ranges) => render_terminal_lines(
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
                    hyperlinks: &snapshot.hyperlinks,
                },
                Some(ranges),
            ),
        }
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
        // - Selection endpoint: thicker 4px bar at 0.9 alpha — the user is
        //   actively growing a selection and needs to see exactly which end
        //   will move when they press h/j/k/l or after toggling with `o`.
        //
        // Both modes draw at the left edge of the cell so there is no visual
        // jump when pressing `v` to transition from caret-only to selection.
        if let Some(caret) = &overlay.caret {
            let visible_row = caret.row.min(fitted_rows.saturating_sub(1));
            let visible_col = caret.col.min(fitted_cols.saturating_sub(1));
            let y = rect.y + visible_row as f32 * cell_h;
            let x = rect.x + visible_col as f32 * cell_w;
            let (caret_w, caret_alpha) = if caret.is_selection_endpoint {
                (4.0, 0.9)
            } else {
                (2.0, 0.8)
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
    dirty_rows: Option<&[TerminalRowRange]>,
) {
    let TerminalRenderLayout {
        cell_w,
        cell_h,
        cols,
        rows,
        style,
        rect,
        hyperlinks,
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
    // The outer pane shell owns border/radius/highlight only. Terminal transparency
    // (`surface_alpha`) applies ONLY to this default/base background, so the frost shows
    // through the empty terminal — explicit program-set cell backgrounds stay opaque (see
    // the cell-bg loop below).
    let surface_bg = with_surface_alpha(default_bg, style.surface_alpha);
    let full_redraw = dirty_rows.is_none();
    if full_redraw && surface_bg[3] > 0.0 {
        primitive_renderer.draw_rect(px, py, pw, ph, surface_bg);
    }
    let max_rows = fitted_rows.min(rows).min(lines.len());
    let max_cols = fitted_cols.min(cols);
    if max_rows == 0 || max_cols == 0 {
        return;
    }

    let full_rows;
    let dirty_ranges = if let Some(ranges) = dirty_rows {
        ranges
    } else {
        full_rows = [TerminalRowRange::new(0, max_rows)];
        &full_rows
    };

    for range in dirty_ranges {
        let row_start = range.start.min(max_rows);
        let row_end = range.end.min(max_rows);
        if row_start >= row_end {
            continue;
        }

        if !full_redraw && surface_bg[3] > 0.0 {
            let y = py + row_start as f32 * cell_h;
            let h = (row_end - row_start) as f32 * cell_h;
            primitive_renderer.draw_rect(px, y, pw, h, surface_bg);
        }

        for (row, line) in lines
            .iter()
            .enumerate()
            .skip(row_start)
            .take(row_end - row_start)
        {
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
                    // Explicit (program-set) backgrounds stay opaque — terminal transparency
                    // applies only to the default background (above). Otherwise a TUI's own
                    // solid fills (e.g. an opaque nvim colorscheme) get washed out by the
                    // frost, badly so over a light theme. Matches kitty/wezterm/iterm.
                    if bg_color[3] > 0.0 {
                        let x = px + bg_start as f32 * cell_w;
                        let w = (bg_end - bg_start) as f32 * cell_w;
                        primitive_renderer.draw_rect(x, y, w, cell_h, bg_color);
                    }
                }
                bg_start = bg_end;
            }

            // OSC 8 hyperlinks on this row (usually none): link cells are recolored
            // and decorated. Cheap fast-path: only scan per-cell when the row has any.
            let row_has_links = hyperlinks.iter().any(|span| span.row == row);

            // Text: group consecutive same-style cells into one shaped run so the
            // shaper sees whole tokens (e.g. `->`) and OpenType ligatures can form.
            // Underlines and box/powerline symbols stay per-cell (drawn via
            // primitives). A blank or symbol cell, a style change, or a column gap
            // flushes the current run; `flush_text_run` queues it cell-snapped.
            let mut run: Option<TerminalTextRun> = None;
            let mut col = 0usize;
            while col < visible_cols {
                let cell = &line.cells[col];
                let cw = cell.width.max(1);
                let x = px + col as f32 * cell_w;
                let width = cw as f32 * cell_w;

                if cell.text.trim().is_empty() {
                    flush_text_run(text_renderer, &mut run, px, y, cell_w, cell_h, style);
                    col += cw;
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
                    flush_text_run(text_renderer, &mut run, px, y, cell_w, cell_h, style);
                    draw_underline_style(
                        primitive_renderer,
                        cell.underline,
                        x,
                        y,
                        width,
                        cell_h,
                        cell.fg,
                    );
                    col += cw;
                    continue;
                }

                // Resolve the per-style family once for this cell, then decide
                // real-italic vs synthesized oblique: a distinct italic family was
                // resolved only when it differs from `normal` (fallback). This
                // generalizes the old `italic_family != font_family` check to all
                // four style slots.
                let resolved = style.families.resolve(cell.bold, cell.italic);
                let is_faux_italic = cell.italic && resolved == style.families.normal;
                let cell_style = TerminalRunStyle {
                    family: resolved,
                    bold: cell.bold,
                    italic: cell.italic && !is_faux_italic,
                    faux_italic: is_faux_italic,
                };

                // Hyperlink cells: recolor the text (and its underline/decoration)
                // to the link color unless the style is `None`.
                let is_link = row_has_links
                    && hyperlinks
                        .iter()
                        .any(|s| s.row == row && col >= s.start_col && col < s.end_col);
                let fg = if is_link && style.hyperlink_style != HyperlinkDecor::None {
                    style.hyperlink_color
                } else {
                    cell.fg
                };

                // Group by font style only — NOT foreground color. Ligatures must
                // form across color boundaries (e.g. a syntax-highlighted operator
                // whose halves are tinted differently), matching kitty/WezTerm; the
                // emission then colors each glyph by its own cell.
                match run.as_mut() {
                    Some(r) if r.next_col == col && r.style == cell_style => {
                        r.push_cell(&cell.text, cw, fg);
                    }
                    _ => {
                        flush_text_run(text_renderer, &mut run, px, y, cell_w, cell_h, style);
                        let mut r = TerminalTextRun::new(col, cell_style);
                        r.push_cell(&cell.text, cw, fg);
                        run = Some(r);
                    }
                }

                draw_underline_style(primitive_renderer, cell.underline, x, y, width, cell_h, fg);
                if is_link {
                    draw_hyperlink_decoration(
                        primitive_renderer,
                        style.hyperlink_style,
                        x,
                        y,
                        width,
                        cell_h,
                        style.hyperlink_color,
                    );
                }
                col += cw;
            }
            flush_text_run(text_renderer, &mut run, px, y, cell_w, cell_h, style);
        }
    }
}

/// Style identity for terminal run grouping: consecutive cells sharing it (and
/// contiguous columns) shape together so OpenType ligatures can form. Only
/// font-face attributes are included — foreground color is deliberately excluded
/// so ligatures span color boundaries (the emission colors each glyph per cell).
#[derive(Clone, Copy, PartialEq, Debug)]
struct TerminalRunStyle<'a> {
    family: &'a str,
    bold: bool,
    italic: bool,
    faux_italic: bool,
}

/// Accumulator for one in-progress terminal text run.
struct TerminalTextRun<'a> {
    /// First column of the run (relative to the row), for the run origin x.
    start_col: usize,
    /// Next expected column; a gap (skipped blank/wide cell) breaks the run.
    next_col: usize,
    /// Column offset of the next cell within the run (sum of prior cell widths).
    col_off: u16,
    text: String,
    /// Per-byte cell-column offset within the run (see `TerminalRun::byte_cols`).
    byte_cols: Vec<u16>,
    /// Per-column foreground color within the run; a wide cell fills each of its
    /// columns. Indexed by run-relative column so the emission can color each
    /// glyph by the cell it snaps to.
    col_colors: Vec<[f32; 4]>,
    style: TerminalRunStyle<'a>,
}

impl<'a> TerminalTextRun<'a> {
    fn new(start_col: usize, style: TerminalRunStyle<'a>) -> Self {
        Self {
            start_col,
            next_col: start_col,
            col_off: 0,
            text: String::new(),
            byte_cols: Vec::new(),
            col_colors: Vec::new(),
            style,
        }
    }

    /// Append one cell's text, tagging each of its bytes with the run-relative
    /// column it occupies and each column with the cell's color, then advancing
    /// past its display width.
    fn push_cell(&mut self, text: &str, cw: usize, fg: [f32; 4]) {
        for _ in 0..text.len() {
            self.byte_cols.push(self.col_off);
        }
        for _ in 0..cw {
            self.col_colors.push(fg);
        }
        self.text.push_str(text);
        self.col_off += cw as u16;
        self.next_col += cw;
    }
}

/// Queue the accumulated run (if any) as one shaped, cell-snapped command, then
/// clear the accumulator.
fn flush_text_run(
    text_renderer: &mut TextRenderer,
    run: &mut Option<TerminalTextRun<'_>>,
    px: f32,
    y: f32,
    cell_w: f32,
    cell_h: f32,
    style: TerminalStyle<'_>,
) {
    let Some(r) = run.take() else {
        return;
    };
    if r.text.is_empty() {
        return;
    }
    let x = px + r.start_col as f32 * cell_w;
    text_renderer.queue_terminal_run(
        &r.text,
        TextBox {
            x,
            y,
            w: r.col_off as f32 * cell_w,
            h: cell_h,
        },
        style.font_size,
        TextStyle {
            // Per-glyph color is supplied via `col_colors`; this is only the
            // fallback used for any glyph whose column can't be resolved.
            color: r.col_colors.first().copied().unwrap_or([1.0; 4]),
            bold: r.style.bold,
            italic: r.style.italic,
            faux_italic: r.style.faux_italic,
            font_family: Some(r.style.family),
        },
        cell_w,
        &r.byte_cols,
        &r.col_colors,
        style.ligatures,
    );
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
    let mut cursor_color = snapshot.cursor_color;
    if cursor_color[3] <= f32::EPSILON {
        cursor_color = snapshot.default_fg;
        cursor_color[3] = 0.85;
    }
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

#[cfg(test)]
mod tests {
    use super::{TerminalRunStyle, TerminalTextRun};

    fn run_style() -> TerminalRunStyle<'static> {
        TerminalRunStyle {
            family: "Maple Mono Normal NF",
            bold: false,
            italic: false,
            faux_italic: false,
        }
    }

    const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

    #[test]
    fn run_push_cell_maps_each_byte_to_its_cell_column() {
        // "->" : two single-width ASCII cells. Each byte maps to its own column,
        // and the run advances one column per cell. The whole text is what the
        // shaper sees, so `calt`/`liga` can collapse it into a ligature — which
        // then snaps back to its first cell at emission.
        let mut run = TerminalTextRun::new(5, run_style());
        run.push_cell("-", 1, WHITE);
        run.push_cell(">", 1, WHITE);
        assert_eq!(run.text, "->");
        assert_eq!(run.byte_cols, vec![0, 1]);
        assert_eq!(run.col_colors, vec![WHITE, WHITE]);
        assert_eq!(run.col_off, 2);
        assert_eq!(run.start_col, 5);
        assert_eq!(run.next_col, 7);
    }

    #[test]
    fn run_push_cell_tags_multibyte_and_wide_cells() {
        // A wide (2-column) CJK cell followed by a single-width cell. The wide
        // cell's 3 UTF-8 bytes all map to column 0 and it fills 2 color columns;
        // the next cell starts at column 2 (not 1), keeping the grid snap correct.
        let mut run = TerminalTextRun::new(0, run_style());
        run.push_cell("世", 2, WHITE);
        run.push_cell("x", 1, RED);
        assert_eq!(run.byte_cols, vec![0, 0, 0, 2]);
        assert_eq!(run.col_colors, vec![WHITE, WHITE, RED]);
        assert_eq!(run.col_off, 3);
        assert_eq!(run.next_col, 3);
    }

    #[test]
    fn run_push_cell_keeps_per_column_color_across_a_color_boundary() {
        // Color is NOT a run boundary: a token tinted differently per cell still
        // joins one run so its ligature can form; each column keeps its own color.
        let mut run = TerminalTextRun::new(0, run_style());
        run.push_cell("=", 1, RED);
        run.push_cell(">", 1, WHITE);
        assert_eq!(run.text, "=>");
        assert_eq!(run.col_colors, vec![RED, WHITE]);
    }

    #[test]
    fn run_style_equality_gates_grouping_by_font_only() {
        // Font-face attributes gate grouping; color does not (it isn't a field).
        let a = run_style();
        assert_eq!(a, run_style());
        let mut bold = run_style();
        bold.bold = true;
        assert_ne!(a, bold);
        let mut italic = run_style();
        italic.italic = true;
        assert_ne!(a, italic);
    }
}
