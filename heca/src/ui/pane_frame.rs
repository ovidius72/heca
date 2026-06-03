use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use heca_config::theme::Theme;

pub struct PaneFrame {
    pub total_rect: [f32; 4], // x, y, w, h
}

impl PaneFrame {
    pub fn get_close_button_rect(&self, theme: &Theme) -> Option<[f32; 4]> {
        let [x, y, w, _h] = self.total_rect;
        let style = &theme.pane_style;
        if !style.show_pane_title { return None; }
        
        let active_bw = 0.0; // approximate
        let fs = style.pane_title_font_size;
        let btn_x = x + w - style.padding - fs;
        let btn_y = y - fs - active_bw;
        
        Some([btn_x, btn_y, fs, fs])
    }

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
        
        let border_color = if focused { style.active_border_color.to_f32x4() } else { style.border_color.to_f32x4() };
        let active_bw = if focused { style.active_border_width } else { style.border_width };

        // 1. Draw Body (Flat)
        primitive.draw_rounded_rect(
            x, y, w, h,
            theme.background.to_f32x4(),
            theme.background.to_f32x4(),
            style.border_radius
        );
        
        // 2. Draw Border (solid rounded)
        primitive.draw_rounded_rect(
            x, y, w, h,
            border_color, border_color,
            style.border_radius
        );
        
        // 3. Hollow out border interior
        if active_bw > 0.0 {
            primitive.draw_rounded_rect(
                x + active_bw, y + active_bw, w - 2.0 * active_bw, h - 2.0 * active_bw,
                theme.background.to_f32x4(), theme.background.to_f32x4(),
                style.border_radius - active_bw
            );
        }

        // 4. Title & close [x] — right-aligned, sitting on the top border
        if style.show_pane_title {
            let fs = style.pane_title_font_size;
            let title_color = if focused { style.pane_title_active_color.to_f32x4() } else { style.pane_title_color.to_f32x4() };
            let bg = theme.background.to_f32x4();
            
            let x_w = fs * 0.7; // width of the × character
            
            // Title at left, [x] at right — with corner margin
            let title_x = x + 8.0;
            let close_x = x + w - x_w - 8.0;
            
            // Both sit on the top border edge
            let text_y = y - fs / 2.0 + 3.0;
            
            // Background strip behind title to "cut" the border
            let title_w = pane_name.len() as f32 * fs * 0.6;
            primitive.draw_rect(title_x - 2.0, text_y, title_w + 4.0, fs, bg);
            
            // Background strip behind [x]
            primitive.draw_rect(close_x - 2.0, text_y, x_w + 4.0, fs, bg);
            
            // Title text (left)
            text.queue_text(pane_name, title_x, text_y, fs, title_color);
            
            // Close [x] (right)
            text.queue_text("×", close_x, text_y, fs * 1.1, title_color);
        }

        // 5. Content area
        let inner_x = x + active_bw + style.padding;
        let inner_y = y + active_bw + style.padding;
        let inner_w = w - 2.0 * (active_bw + style.padding);
        let inner_h = h - 2.0 * (active_bw + style.padding);
        
        [inner_x, inner_y, inner_w, inner_h]
    }
}
