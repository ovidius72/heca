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

        let bw = style.border_width;
        let fs = style.pane_title_font_size;
        let btn_x = x + w - bw - style.padding - fs * 1.0;
        let btn_y = y - bw - fs + 2.0;
        
        Some([btn_x, btn_y, fs, fs])
    }

    pub fn render(
        &self,
        theme: &Theme,
        focused: bool,
        pane_name: &str,
        primitive: &mut PrimitiveRenderer,
        text: &mut TextRenderer,
    ) -> [f32; 4] { // Returns the inner content rectangle
        let [x, y, w, h] = self.total_rect;
        let style = &theme.pane_style;
        
        let border_color = if focused { style.active_border_color.to_f32x4() } else { style.border_color.to_f32x4() };
        let active_bw = if focused { style.active_border_width } else { style.border_width };

        // 1. Draw Body (Flat)
        primitive.draw_rounded_rect(
            x, y, w, h,
            style.bg_gradient_start.to_f32x4(), 
            style.bg_gradient_end.to_f32x4(), 
            style.border_radius
        );
        
        // 2. Draw Border (Solid)
        primitive.draw_rounded_rect(
            x, y, w, h,
            border_color, border_color,
            style.border_radius
        );
        
        // 3. Hollow out the border with a solid color
        primitive.draw_rounded_rect(
            x + active_bw, y + active_bw, w - 2.0 * active_bw, h - 2.0 * active_bw,
            theme.background.to_f32x4(), theme.background.to_f32x4(),
            style.border_radius - active_bw
        );

        // 4. Floating Title (above the border)
        if style.show_pane_title {
            let title_color = if focused { style.pane_title_active_color.to_f32x4() } else { style.pane_title_color.to_f32x4() };
            let fs = style.pane_title_font_size;
            
            // Title text at the top-left, floating above border
            text.queue_text(
                pane_name,
                x + style.padding,
                y - fs,
                fs,
                title_color,
            );
            
            // Close button [x] at the top-right, floating above border, no background
            text.queue_text(
                "×",
                x + w - style.padding - fs + 2.0,
                y - fs + 2.0,
                fs,
                title_color,
            );
        }

        // 5. Content Area Calculation
        let inner_x = x + active_bw + style.padding;
        let inner_w = w - 2.0 * (active_bw + style.padding);
        let inner_h = (h - 2.0 * active_bw) - 2.0 * style.padding;
        
        [inner_x, y + active_bw + style.padding, inner_w, inner_h]
    }
}
