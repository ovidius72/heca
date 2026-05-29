use heca_core::types::Rect;

/// Layout configuration for chrome elements around the pane area.
#[derive(Clone, Copy, Debug)]
pub struct ChromeConfig {
    pub tab_bar_height: f32,
    pub status_bar_height: f32,
    pub left_sidebar_width: f32,
    pub right_sidebar_width: f32,
}

impl ChromeConfig {
    /// Compute the rectangle available for pane content, given a window size.
    /// Chrome occupies the outer edges; panes get the center.
    pub fn content_rect(&self, window_width: f32, window_height: f32) -> Rect {
        let y = self.tab_bar_height;
        let x = self.left_sidebar_width;
        let w = window_width - self.left_sidebar_width.min(window_width)
            - self.right_sidebar_width.min(window_width - self.left_sidebar_width);
        let h = window_height - self.tab_bar_height - self.status_bar_height;
        Rect::new(x, y, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_rect_full() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 200.0,
            right_sidebar_width: 200.0,
        };
        let r = c.content_rect(1280.0, 800.0);
        assert_eq!(r.x, 200.0);
        assert_eq!(r.y, 32.0);
        assert_eq!(r.w, 880.0);
        assert_eq!(r.h, 744.0);
    }

    #[test]
    fn test_content_rect_no_sidebars() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 0.0,
            right_sidebar_width: 0.0,
        };
        let r = c.content_rect(1024.0, 768.0);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 32.0);
        assert_eq!(r.w, 1024.0);
        assert_eq!(r.h, 712.0);
    }
}
