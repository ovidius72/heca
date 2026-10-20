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

    draw_box_drawing_cell(primitive_renderer, ch, x, y, w, h, color)
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
