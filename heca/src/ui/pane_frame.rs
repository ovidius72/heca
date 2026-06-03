use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use heca_config::theme::{Theme, PaneStyle};

/// Margin from the pane corner for title/[x] placement.
const CORNER_MARGIN: f32 = 8.0;
/// Extra padding around the close [x] for hit-testing.
const CLOSE_HIT_PAD: f32 = 4.0;
/// Approximate glyph-to-width factor (will be replaced when TextRenderer exposes metrics).
const GLYPH_WIDTH_FACTOR: f32 = 0.6;
/// Multiplier for the [x] character size relative to the title font size.
const CLOSE_BTN_SCALE: f32 = 1.1;

/// Renders the chrome frame around a pane: body, border, floating title, and close button.
///
/// `PaneFrame` is a stateless component. Call `render()` during each frame to draw
/// the pane and obtain the inner content rectangle for rendering backend data.
///
/// ```
/// let frame = PaneFrame { total_rect: [px, py, pw, ph] };
/// let [ix, iy, iw, ih] = frame.render(&theme, is_focused, &name, &mut prim, &mut text);
/// ```
pub struct PaneFrame {
    pub total_rect: [f32; 4], // [x, y, width, height] of the pane area (border included)
}

// ── Shared layout helpers ─────────────────────────────────────────────────────
impl PaneFrame {
    /// Returns the approximate pixel width of a string at a given font size.
    fn text_width(text: &str, font_size: f32) -> f32 {
        text.len() as f32 * font_size * GLYPH_WIDTH_FACTOR
    }

    /// Returns the position and size of the title text and close [x] glyph.
    /// Both sit on the top border line, right-aligned.
    fn title_layout(&self, style: &PaneStyle) -> Option<TitleLayout> {
        if !style.show_pane_title {
            return None;
        }
        let [x, y, w, _h] = self.total_rect;
        let fs = style.pane_title_font_size;

        let x_w = fs * 0.7; // approximate width of the × character
        let title_x = x + CORNER_MARGIN;
        let close_x = x + w - x_w - CORNER_MARGIN;
        let text_y = y - fs / 2.0 + 3.0; // sits on the top border edge

        Some(TitleLayout {
            title_x,
            close_x,
            close_w: x_w + CLOSE_HIT_PAD,
            text_y,
            font_size: fs,
        })
    }
}

// ── Public API ────────────────────────────────────────────────────────────────
impl PaneFrame {
    /// Returns the clickable hitbox for the close [x] button, if visible.
    pub fn get_close_button_rect(&self, theme: &Theme) -> Option<[f32; 4]> {
        let layout = self.title_layout(&theme.pane_style)?;
        Some([layout.close_x, layout.text_y, layout.close_w, layout.font_size])
    }

    /// Renders the pane frame layer onto the GPU primitive/text queues.
    ///
    /// Returns the `[inner_x, inner_y, inner_w, inner_h]` content rectangle
    /// that backends should render into.
    pub fn render(
        &self,
        theme: &Theme,
        focused: bool,
        pane_name: &str,
        primitive: &mut PrimitiveRenderer,
        text: &mut TextRenderer,
    ) -> [f32; 4] {
        let [x, y, w, h] = self.total_rect;
        let style = &theme.pane_style;

        let border_color = if focused {
            style.active_border_color.to_f32x4()
        } else {
            style.border_color.to_f32x4()
        };
        let bw = if focused {
            style.active_border_width
        } else {
            style.border_width
        };
        let bg = theme.background.to_f32x4();

        // 1. Body + Border (single call, no hollow-out duplication)
        primitive.draw_rounded_border(x, y, w, h, border_color, bw, style.border_radius, bg);

        // 2. Floating title + close [x]
        if let Some(layout) = self.title_layout(style) {
            let title_color = if focused {
                style.pane_title_active_color.to_f32x4()
            } else {
                style.pane_title_color.to_f32x4()
            };

            // Background strips that mask the border underneath each text element
            let title_w = Self::text_width(pane_name, layout.font_size);
            primitive.draw_rect(layout.title_x - 2.0, layout.text_y, title_w + 4.0, layout.font_size, bg);
            primitive.draw_rect(layout.close_x - 2.0, layout.text_y, layout.close_w + 4.0, layout.font_size, bg);

            text.queue_text(pane_name, layout.title_x, layout.text_y, layout.font_size, title_color);
            text.queue_text("×", layout.close_x, layout.text_y, layout.font_size * CLOSE_BTN_SCALE, title_color);
        }

        // 3. Content area
        let inner_x = x + bw + style.padding;
        let inner_y = y + bw + style.padding;
        let inner_w = w - 2.0 * (bw + style.padding);
        let inner_h = h - 2.0 * (bw + style.padding);

        [inner_x, inner_y, inner_w, inner_h]
    }
}

// ── Internal types ────────────────────────────────────────────────────────────
struct TitleLayout {
    title_x: f32,
    close_x: f32,
    close_w: f32,   // hitbox width for the [x]
    text_y: f32,
    font_size: f32,
}
