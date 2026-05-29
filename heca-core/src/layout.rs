use crate::types::Rect;

/// A mock pane for Phase 1 — placeholder content with visual properties
#[derive(Clone, Debug)]
pub struct MockPane {
    pub id: usize,
    pub title: String,
    pub background: [f32; 4],
}

impl MockPane {
    pub fn new(id: usize, title: &str, background: [f32; 4]) -> Self {
        Self {
            id,
            title: title.to_string(),
            background,
        }
    }
}

/// A simple mock tiling layout for Phase 1 visual validation
#[derive(Clone, Debug)]
pub struct MockLayout {
    pub panes: Vec<MockPane>,
    pub float_pane: Option<MockPane>,
}

impl MockLayout {
    pub fn default_demo() -> Self {
        Self {
            panes: vec![
                MockPane::new(1, "Editor", hex_to_f32x4("#1e1e2e")),
                MockPane::new(2, "Terminal", hex_to_f32x4("#181825")),
                MockPane::new(3, "Files", hex_to_f32x4("#11111b")),
            ],
            float_pane: Some(MockPane::new(4, "Preview", hex_to_f32x4("#313244"))),
        }
    }

    /// Compute pane rectangles for a simple tiling layout:
    /// - Left 60%: one tall pane (Editor)
    /// - Right 40%: split horizontally (Terminal top, Files bottom)
    /// - Centered float: Preview (300x200)
    pub fn compute_rects(
        &self,
        width: f32,
        height: f32,
    ) -> (Vec<(Rect, &MockPane)>, Option<(Rect, &MockPane)>) {
        let mut result = Vec::new();

        let left_width = width * 0.6;
        let right_width = width * 0.4;
        let right_half_height = height / 2.0;

        // Pane 1: Editor — left 60%, full height
        if let Some(pane) = self.panes.first() {
            result.push((
                Rect::new(0.0, 0.0, left_width, height),
                pane,
            ));
        }

        // Pane 2: Terminal — top-right 40%, 50% height
        if let Some(pane) = self.panes.get(1) {
            result.push((
                Rect::new(left_width, 0.0, right_width, right_half_height),
                pane,
            ));
        }

        // Pane 3: Files — bottom-right 40%, 50% height
        if let Some(pane) = self.panes.get(2) {
            result.push((
                Rect::new(left_width, right_half_height, right_width, right_half_height),
                pane,
            ));
        }

        // Float pane: Preview — centered 300x200
        let float_rect = self.float_pane.as_ref().map(|pane| {
            let fw = 300.0f32.min(width * 0.5);
            let fh = 200.0f32.min(height * 0.5);
            let fx = (width - fw) / 2.0;
            let fy = (height - fh) / 2.0;
            (Rect::new(fx, fy, fw, fh), pane)
        });

        (result, float_rect)
    }
}

/// Convert a hex color string (#RRGGBB or #RRGGBBAA) to [f32; 4] in RGBA
pub fn hex_to_f32x4(hex: &str) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');
    let len = hex.len();

    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    let a = if len >= 8 {
        u8::from_str_radix(&hex[6..8], 16).unwrap_or(255)
    } else {
        255
    };

    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}
