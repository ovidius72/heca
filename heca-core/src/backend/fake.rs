//! Fake backend for testing layout performance without PTY overhead.

use super::{BackendRenderData, PaneBackend, PaneType, TerminalCell, TerminalLine};

/// A fake backend that renders a static test pattern without any I/O.
pub struct FakeBackend {
    title: String,
    cols: usize,
    rows: usize,
}

impl FakeBackend {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            title: "Fake".to_string(),
            cols,
            rows,
        }
    }
}

impl PaneBackend for FakeBackend {
    fn pane_type(&self) -> PaneType {
        PaneType::Terminal
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn set_size(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
    }

    fn process_input(&mut self, _data: &[u8]) {
        // No-op
    }

    fn update(&mut self) -> bool {
        false // never has "new" data since it's static
    }

    fn render_data(&self) -> BackendRenderData {
        // Minimal: just 2 lines of text to avoid cosmic-text overhead.
        // Full grid rendering is too slow for layout testing.
        let mut lines = Vec::with_capacity(2);
        let mut cells = Vec::with_capacity(self.cols);
        for _ in 0..self.cols {
            cells.push(TerminalCell {
                c: ' ',
                fg: [0.9, 0.9, 0.9, 1.0],
                bg: [0.05, 0.05, 0.08, 1.0],
                bold: false,
            });
        }
        lines.push(TerminalLine {
            cells: cells.clone(),
        });
        // Second line has a short label so queue_text only gets called once
        if self.cols >= 8 {
            let label = "FakePane";
            for (i, c) in label.chars().enumerate() {
                cells[i].c = c;
            }
        }
        lines.push(TerminalLine { cells });

        let (cell_w, cell_h) = self.cell_size();
        BackendRenderData::Terminal {
            lines,
            cursor_col: 0,
            cursor_row: 0,
            cell_w,
            cell_h,
        }
    }

    fn should_close(&self) -> bool {
        false
    }
}
