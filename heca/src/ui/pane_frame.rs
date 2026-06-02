use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use heca_config::theme::Theme;

pub struct PaneFrame {
    pub total_rect: [f32; 4], // x, y, w, h
}

impl PaneFrame {
    pub fn get_close_button_rect(&self, theme: &Theme) -> Option<[f32; 4]> {
        let [x, y, w, h] = self.total_rect;
        let style = &theme.pane_style;
        if !style.show_title_bar { return None; }

        let bw = style.border_width;
        let th = style.title_bar_height;
        let close_btn_size = (th * 0.7).clamp(12.0, 18.0);
        let btn_x = x + w - bw - close_btn_size - style.padding;
        let btn_y = y + bw + (th - close_btn_size) / 2.0;
        
        Some([btn_x, btn_y, close_btn_size, close_btn_size])
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
        let glow_color = style.glow_color.to_f32x4();
        
        // 1. Draw Bloom/Glow
        primitive.draw_glow_rounded_rect(x, y, w, h, glow_color, style.border_radius, style.glow_radius);
        
        // 2. Draw Background Gradient
        primitive.draw_rounded_rect(
            x, y, w, h, 
            style.bg_gradient_start.to_f32x4(), 
            style.bg_gradient_end.to_f32x4(), 
            style.border_radius
        );
        
        // 3. Draw Neon Border
        primitive.draw_rounded_rect(
            x, y, w, h,
            border_color, border_color,
            style.border_radius
        );
        // Note: The current draw_rounded_rect fills the rect. 
        // To do a true border, we'd need a dedicated border method.
        // Let's use the draw_border method if available, but with rounded corners it's tricky.
        // For now, let's just draw a slightly smaller inner rect with the bg color to "hollow out" the border.
        primitive.draw_rounded_rect(
            x + active_bw, y + active_bw, w - 2.0 * active_bw, h - 2.0 * active_bw,
            style.bg_gradient_start.to_f32x4(), style.bg_gradient_end.to_f32x4(),
            style.border_radius - active_bw
        );

        // 4. Title Bar
        let mut inner_y = y + active_bw;
        if style.show_title_bar {
            let th = style.title_bar_height;
            let tb_color = style.title_bar_bg_color.to_f32x4();
            
            // Title bar background
            primitive.draw_rect(x + active_bw, inner_y, w - 2.0 * active_bw, th, tb_color);
            
            // Title text
            let title_color = if focused { style.title_active_color.to_f32x4() } else { style.title_color.to_f32x4() };
            text.queue_text(
                pane_name, 
                x + active_bw + style.padding, 
                inner_y + (th - style.title_font_size) / 2.0 + 1.0, 
                style.title_font_size, 
                title_color
            );
            
            // Close Button [x]
            let close_btn_size = (th * 0.7).clamp(12.0, 18.0);
            let btn_x = x + w - active_bw - close_btn_size - style.padding;
            let btn_y = inner_y + (th - close_btn_size) / 2.0;
            
            primitive.draw_rounded_rect(
                btn_x, btn_y, close_btn_size, close_btn_size,
                [0.2, 0.2, 0.2, 1.0], [0.4, 0.4, 0.4, 1.0], 2.0
            );
            
            // Precisely center the 'x' character
            text.queue_text("×", btn_x + (close_btn_size / 2.0) - 2.5, btn_y + (close_btn_size / 2.0) - 3.5, 10.0, title_color);
            
            inner_y += th;
        }

        // 5. Content Area Calculation
        let inner_x = x + active_bw + style.padding;
        let inner_w = w - 2.0 * (active_bw + style.padding);
        let inner_h = (h - 2.0 * active_bw) - (if style.show_title_bar { style.title_bar_height } else { 0.0 }) - 2.0 * style.padding;
        
        [inner_x, inner_y + style.padding, inner_w, inner_h]
    }
}
