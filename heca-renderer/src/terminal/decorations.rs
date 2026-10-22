use super::*;
use crate::primitive::PrimitiveRenderer;
use crate::text::TextBox;
use heca_core::backend::TerminalUnderlineStyle;

/// Draw the hyperlink decoration for one link cell (straight underline or
/// undercurl, in the link color). `None`/`Color` draw nothing extra — the recolor
/// alone marks the link.
pub(super) fn draw_hyperlink_decoration(
    primitive_renderer: &mut PrimitiveRenderer,
    style: HyperlinkDecor,
    x: f32,
    y: f32,
    width: f32,
    cell_h: f32,
    color: [f32; 4],
) {
    let underline = match style {
        HyperlinkDecor::Underline => TerminalUnderlineStyle::Single,
        HyperlinkDecor::Undercurl => TerminalUnderlineStyle::Curly,
        HyperlinkDecor::None | HyperlinkDecor::Color => return,
    };
    draw_underline_style(primitive_renderer, underline, x, y, width, cell_h, color);
}

pub(super) fn draw_underline_style(
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
            primitive_renderer.draw_rect(x, (baseline_y - separation).max(y), width, stroke, color);
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

pub(super) fn draw_terminal_symbol_cell(
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

    if draw_block_cell(primitive_renderer, ch, x, y, w, h, color) {
        return true;
    }

    draw_box_drawing_cell(primitive_renderer, ch, x, y, w, h, color)
}

/// One cell-filling rectangle for a Block Element: `(x, y, w, h)`.
pub(super) type BlockRect = (f32, f32, f32, f32);
/// The cell-filling rectangles of a **Block Element** (`U+2580`–`U+259F`),
/// exactly as Alacritty/Kitty draw them: edge to edge in the *cell's* geometry,
/// never the font's — a font's block glyphs stop short of the cell bounds
/// (advance, hinting, ascent), so two same-colour blocks hand-rendered by the
/// font show a hairline seam between them. Drawing the cell's own rectangles
/// makes `█▄▀▌▐▙▟…` tile perfectly, which is what logos and ASCII art made of
/// blocks need.
///
/// The shading characters (`░` `▒` `▓`) stay with the font: they are dithered
/// textures, not solid fills, and the font draws them without seams at normal
/// scale.
pub(super) fn block_cell_rects(ch: char, x: f32, y: f32, w: f32, h: f32) -> Option<Vec<BlockRect>> {
    // The cell cut in halves, for the quadrant characters (U+2596–U+259F).
    let ul = (x, y, w * 0.5, h * 0.5);
    let ur = (x + w * 0.5, y, w * 0.5, h * 0.5);
    let ll = (x, y + h * 0.5, w * 0.5, h * 0.5);
    let lr = (x + w * 0.5, y + h * 0.5, w * 0.5, h * 0.5);

    let rects = match ch {
        // U+2580 ▀ upper half
        '\u{2580}' => vec![(x, y, w, h * 0.5)],
        // U+2581–U+2587 ▁ ▂ ▃ ▄ ▅ ▆ ▇ — the lower n·⅛ of the cell
        '\u{2581}'..='\u{2587}' => {
            let n = (ch as u32 - 0x2580) as f32;
            vec![(x, y + h * (8.0 - n) / 8.0, w, h * n / 8.0)]
        }
        // U+2588 █ full
        '\u{2588}' => vec![(x, y, w, h)],
        // U+2589–U+258B ▉ ▊ ▋ — the left 7·⅛, 6·⅛, 5·⅛ strips
        '\u{2589}'..='\u{258B}' => {
            let n = (0x2590 - ch as u32) as f32;
            vec![(x, y, w * n / 8.0, h)]
        }
        // U+258C ▌ left half
        '\u{258C}' => vec![(x, y, w * 0.5, h)],
        // U+258D–U+258F ▍ ▎ ▏ — the left 3·⅛, 2·⅛, 1·⅛ strips
        '\u{258D}'..='\u{258F}' => {
            let n = (0x2590 - ch as u32) as f32;
            vec![(x, y, w * n / 8.0, h)]
        }
        // U+2590 ▐ right half
        '\u{2590}' => vec![(x + w * 0.5, y, w * 0.5, h)],
        // U+2594 ▔ upper eighth
        '\u{2594}' => vec![(x, y, w, h / 8.0)],
        // U+2595 ▕ right eighth
        '\u{2595}' => vec![(x + w * 7.0 / 8.0, y, w / 8.0, h)],
        // U+2596–U+259F ▖ ▗ ▘ ▝ ▙ ▚ ▛ ▜ ▞ ▟ — their named quadrants
        '\u{2596}' => vec![ll],
        '\u{2597}' => vec![lr],
        '\u{2598}' => vec![ul],
        '\u{2599}' => vec![ul, ll, lr],
        '\u{259A}' => vec![ul, lr],
        '\u{259B}' => vec![ul, ur, ll],
        '\u{259C}' => vec![ul, ur, lr],
        '\u{259D}' => vec![ur],
        '\u{259E}' => vec![ur, ll],
        '\u{259F}' => vec![ur, ll, lr],
        _ => return None,
    };
    Some(rects)
}

/// Draw one Block Element cell from [`block_cell_rects`] — the shared fg colour,
/// every rectangle exact to its cell edge.
fn draw_block_cell(
    primitive_renderer: &mut PrimitiveRenderer,
    ch: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
) -> bool {
    let Some(rects) = block_cell_rects(ch, x, y, w, h) else {
        return false;
    };
    for (rx, ry, rw, rh) in rects {
        primitive_renderer.draw_rect(rx, ry, rw, rh, color);
    }
    true
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
            primitive_renderer.draw_triangle([x + w, y], [x + w, y + h], [x, y + h * 0.5], color);
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
            primitive_renderer.draw_rect(x + inset, band_y, (w - inset).max(0.0), draw_h, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{block_cell_rects, BlockRect};

    /// A cell the tests measure: ten logical px, so an eighth is exact.
    const X: f32 = 0.0;
    const Y: f32 = 0.0;
    const W: f32 = 10.0;
    const H: f32 = 8.0;
    const E: f32 = 1.0; // one eighth of the height

    fn rects(ch: char) -> Vec<BlockRect> {
        block_cell_rects(ch, X, Y, W, H).expect("expected a block element")
    }

    fn none(ch: char) {
        assert!(
            block_cell_rects(ch, X, Y, W, H).is_none(),
            "{ch:?} must stay with the font"
        );
    }

    #[test]
    fn the_upper_and_lower_halves_meet_at_the_cell_middle_with_no_gap() {
        let top = rects('▀');
        let bottom = rects('▄');
        assert_eq!(top, vec![(X, Y, W, 4.0 * E)]);
        assert_eq!(
            bottom,
            vec![(X, Y + 4.0 * E, W, 4.0 * E)],
            "the lower half starts exactly where the upper ends"
        );
    }

    #[test]
    fn the_lower_blocks_stack_by_eighths() {
        assert_eq!(rects('▁'), vec![(X, Y + 7.0 * E, W, 1.0 * E)]);
        assert_eq!(rects('▃'), vec![(X, Y + 5.0 * E, W, 3.0 * E)]);
        assert_eq!(rects('▇'), vec![(X, Y + 1.0 * E, W, 7.0 * E)]);
    }

    #[test]
    fn the_full_block_fills_the_whole_cell() {
        assert_eq!(rects('█'), vec![(X, Y, W, H)]);
    }

    #[test]
    fn the_left_and_right_strips_split_the_width_exactly() {
        assert_eq!(rects('▌'), vec![(X, Y, 5.0, H)]);
        assert_eq!(rects('▐'), vec![(X + 5.0, Y, 5.0, H)]);
        assert_eq!(rects('▏'), vec![(X, Y, 1.25, H)]);
        assert_eq!(rects('▕'), vec![(X + 8.75, Y, 1.25, H)]);
    }

    #[test]
    fn the_quadrants_fill_their_quarter_of_the_cell() {
        let (ul, ur, ll, lr) = (
            (X, Y, 5.0, 4.0),
            (X + 5.0, Y, 5.0, 4.0),
            (X, Y + 4.0, 5.0, 4.0),
            (X + 5.0, Y + 4.0, 5.0, 4.0),
        );
        assert_eq!(rects('▖'), vec![ll], "lower-left quadrant");
        assert_eq!(rects('▗'), vec![lr], "lower-right quadrant");
        assert_eq!(rects('▘'), vec![ul], "upper-left quadrant");
        assert_eq!(rects('▝'), vec![ur], "upper-right quadrant");
        assert_eq!(rects('▚'), vec![ul, lr], "the diagonal pair");
        assert_eq!(
            rects('▟'),
            vec![ur, ll, lr],
            "everything but the upper left"
        );
    }

    #[test]
    fn a_non_block_element_stays_with_the_font() {
        none('a');
        none('─'); // box drawing — `draw_box_drawing_cell` owns it
        none('✻'); // a glyph, not a block — the Claude Code logo's star itself
        none('░');
        none('▒');
        none('▓'); // shades: dithered font textures
    }
}
