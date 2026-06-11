//! Fake backend for testing layout performance without PTY overhead.

use super::{
    BackendRenderData, PaneBackend, PaneType, TerminalCell, TerminalCursor, TerminalDamage,
    TerminalLine, TerminalSnapshot,
};

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

    fn terminal_snapshot(&self) -> Option<TerminalSnapshot> {
        let blank_row = || {
            let mut cells = Vec::with_capacity(self.cols);
            for _ in 0..self.cols {
                cells.push(TerminalCell {
                    c: ' ',
                    fg: [0.9, 0.9, 0.9, 1.0],
                    bg: [0.05, 0.05, 0.08, 1.0],
                    bold: false,
                });
            }
            TerminalLine { cells }
        };

        let mut lines = Vec::with_capacity(self.rows.max(2));
        lines.push(blank_row());

        let mut label_row = blank_row();
        if self.cols >= 8 {
            let label = "FakePane";
            for (i, c) in label.chars().enumerate() {
                label_row.cells[i].c = c;
            }
        }
        lines.push(label_row);

        while lines.len() < self.rows {
            lines.push(blank_row());
        }

        let (cell_w, cell_h) = self.cell_size();
        Some(TerminalSnapshot {
            cols: self.cols,
            rows: self.rows,
            cell_w,
            cell_h,
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: true,
            },
            damage: TerminalDamage::Full,
            lines,
        })
    }

    fn render_data(&self) -> BackendRenderData {
        // Minimal: just 2 lines of text to avoid excessive generic text work in
        // layout/perf-focused scenarios that still use the legacy renderer.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_backend_exposes_terminal_snapshot() {
        let backend = FakeBackend::new(16, 8);
        let snapshot = backend
            .terminal_snapshot()
            .expect("fake backend should expose terminal snapshots");

        assert_eq!(snapshot.cols, 16, "snapshot cols should match backend");
        assert_eq!(snapshot.rows, 8, "snapshot rows should match backend");
        assert_eq!(snapshot.cursor.col, 0, "fake cursor col should stay at origin");
        assert_eq!(snapshot.cursor.row, 0, "fake cursor row should stay at origin");
        assert!(
            matches!(snapshot.damage, TerminalDamage::Full),
            "fake backend should fully invalidate its static snapshot"
        );
    }
}
