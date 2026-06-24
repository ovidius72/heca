//! Fake backend for testing layout performance without PTY overhead.

use super::{
    BackendKeyEvent, BackendMouseEvent, BackendRenderData, PaneBackend, PaneType, TerminalCell,
    TerminalCursor, TerminalCursorShape, TerminalDamage, TerminalLine, TerminalSnapshot,
    TerminalUnderlineStyle,
};
use crate::runtime::PaneRuntime;

/// A fake backend that renders a static test pattern without any I/O.
pub struct FakeBackend {
    title: String,
    cols: usize,
    rows: usize,
    cell_w: f32,
    cell_h: f32,
    dirty: bool,
    /// Settable runtime snapshot for tests (detection lives in `TerminalBackend`).
    runtime: PaneRuntime,
    /// One-shot exit code drained by `take_exit_code` (tests simulate exits).
    pending_exit: Option<i32>,
}

impl FakeBackend {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self::with_cell_size(cols, rows, 8.4, 14.0)
    }

    pub fn with_cell_size(cols: usize, rows: usize, cell_w: f32, cell_h: f32) -> Self {
        Self {
            title: "Fake".to_string(),
            cols,
            rows,
            cell_w,
            cell_h,
            dirty: true,
            runtime: PaneRuntime::default(),
            pending_exit: None,
        }
    }

    /// Set the runtime snapshot this fake reports via [`PaneBackend::runtime`].
    pub fn set_runtime(&mut self, runtime: PaneRuntime) {
        self.runtime = runtime;
    }

    /// Queue an exit code to be drained once via [`PaneBackend::take_exit_code`].
    pub fn queue_exit(&mut self, code: i32) {
        self.pending_exit = Some(code);
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
        if self.cols == cols && self.rows == rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.dirty = true;
    }

    fn process_input(&mut self, _data: &[u8]) {
        // No-op
    }

    fn process_key_event(&mut self, _event: &BackendKeyEvent) -> bool {
        false
    }

    fn process_mouse_event(&mut self, _event: &BackendMouseEvent) -> bool {
        false
    }

    fn set_cell_size(&mut self, cell_w: f32, cell_h: f32) {
        self.cell_w = cell_w;
        self.cell_h = cell_h;
        self.dirty = true;
    }

    fn update(&mut self) -> bool {
        false // never has "new" data since it's static
    }

    fn terminal_snapshot(&self) -> Option<TerminalSnapshot> {
        let blank_row = || {
            let mut cells = Vec::with_capacity(self.cols);
            for _ in 0..self.cols {
                cells.push(TerminalCell {
                    text: " ".to_string(),
                    fg: [0.9, 0.9, 0.9, 1.0],
                    bg: [0.05, 0.05, 0.08, 1.0],
                    bold: false,
                    italic: false,
                    underline: TerminalUnderlineStyle::None,
                    width: 1,
                });
            }
            TerminalLine { cells }
        };

        let mut lines = Vec::with_capacity(self.rows);
        if self.rows > 0 {
            lines.push(blank_row());
        }

        if self.rows > 1 {
            let mut label_row = blank_row();
            if self.cols >= 8 {
                let label = "FakePane";
                for (i, c) in label.chars().enumerate() {
                    label_row.cells[i].text = c.to_string();
                }
            }
            lines.push(label_row);
        }

        while lines.len() < self.rows {
            lines.push(blank_row());
        }

        let (cell_w, cell_h) = self.cell_size();
        let snapshot = TerminalSnapshot {
            cols: self.cols,
            rows: self.rows,
            cell_w,
            cell_h,
            default_fg: [0.9, 0.9, 0.9, 1.0],
            default_bg: [0.05, 0.05, 0.08, 1.0],
            cursor_color: [0.9, 0.9, 0.9, 0.85],
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: self.cols > 0 && self.rows > 0,
                shape: TerminalCursorShape::Bar,
            },
            lines,
            viewport_offset: 0,
            at_bottom: true,
            scrollback_rows: self.rows,
            viewport_top_stable_row: 0,
        };
        snapshot.debug_assert_valid();
        Some(snapshot)
    }

    fn take_terminal_damage(&mut self) -> TerminalDamage {
        if std::mem::replace(&mut self.dirty, false) {
            TerminalDamage::Full
        } else {
            TerminalDamage::None
        }
    }

    fn render_data(&self) -> BackendRenderData {
        // Keep the legacy path intentionally lightweight while the app still
        // consumes `BackendRenderData` directly. The full visible viewport is
        // available through `terminal_snapshot()`, but the old renderer only
        // needs a small placeholder payload for layout/perf scenarios.
        let row_count = self.rows.min(2);
        let mut lines = Vec::with_capacity(row_count);
        let mut cells = Vec::with_capacity(self.cols);
        for _ in 0..self.cols {
            cells.push(TerminalCell {
                text: " ".to_string(),
                fg: [0.9, 0.9, 0.9, 1.0],
                bg: [0.05, 0.05, 0.08, 1.0],
                bold: false,
                italic: false,
                underline: TerminalUnderlineStyle::None,
                width: 1,
            });
        }

        if row_count > 0 {
            lines.push(TerminalLine {
                cells: cells.clone(),
            });
        }

        if row_count > 1 {
            if self.cols >= 8 {
                let label = "FakePane";
                for (i, c) in label.chars().enumerate() {
                    cells[i].text = c.to_string();
                }
            }
            lines.push(TerminalLine { cells });
        }

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

    fn cell_size(&self) -> (f32, f32) {
        (self.cell_w, self.cell_h)
    }

    fn runtime(&self) -> PaneRuntime {
        self.runtime.clone()
    }

    fn take_exit_code(&mut self) -> Option<i32> {
        self.pending_exit.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_backend_runtime_and_exit_round_trip() {
        use crate::runtime::{ContentKind, PaneRuntime, ProcessStatus};
        let mut backend = FakeBackend::new(16, 8);

        // Default runtime before any set.
        assert_eq!(backend.runtime(), PaneRuntime::default());

        let runtime = PaneRuntime {
            program: Some("nvim".into()),
            status: ProcessStatus::Running,
            kind: ContentKind::Terminal,
            ..PaneRuntime::default()
        };
        backend.set_runtime(runtime.clone());
        assert_eq!(backend.runtime(), runtime);

        // Exit code drains exactly once.
        assert_eq!(backend.take_exit_code(), None, "no exit queued ⇒ None");
        backend.queue_exit(7);
        assert_eq!(backend.take_exit_code(), Some(7), "queued exit drains once");
        assert_eq!(
            backend.take_exit_code(),
            None,
            "drained exit does not repeat"
        );
    }

    #[test]
    fn fake_backend_exposes_terminal_snapshot() {
        let mut backend = FakeBackend::new(16, 8);
        let snapshot = backend
            .terminal_snapshot()
            .expect("fake backend should expose terminal snapshots");

        assert_eq!(snapshot.cols, 16, "snapshot cols should match backend");
        assert_eq!(snapshot.rows, 8, "snapshot rows should match backend");
        assert_eq!(
            snapshot.cursor.col, 0,
            "fake cursor col should stay at origin"
        );
        assert_eq!(
            snapshot.cursor.row, 0,
            "fake cursor row should stay at origin"
        );
        assert_eq!(
            snapshot.lines.len(),
            snapshot.rows,
            "snapshot line count should match visible fake backend height"
        );

        let initial_damage = backend.take_terminal_damage();
        assert!(
            matches!(initial_damage, TerminalDamage::Full),
            "first explicit damage read should request a full redraw"
        );

        let second_damage = backend.take_terminal_damage();
        assert!(
            second_damage.is_empty(),
            "unchanged fake backend should stop advertising redraw damage after acknowledgement"
        );
    }

    #[test]
    fn fake_backend_snapshot_respects_short_heights() {
        let empty = FakeBackend::new(16, 0)
            .terminal_snapshot()
            .expect("fake backend should expose terminal snapshots");
        assert_eq!(empty.rows, 0, "zero-height backend should report zero rows");
        assert!(
            empty.lines.is_empty(),
            "zero-height backend should not produce any snapshot rows"
        );

        let single = FakeBackend::new(16, 1)
            .terminal_snapshot()
            .expect("fake backend should expose terminal snapshots");
        assert_eq!(
            single.lines.len(),
            1,
            "single-row backend should only produce one visible row"
        );
        assert!(
            single.cursor.visible,
            "single-row backend should keep the cursor visible when cells exist"
        );

        let mut resized = FakeBackend::new(16, 1);
        let _ = resized
            .terminal_snapshot()
            .expect("fake backend should expose terminal snapshots");
        let _ = resized.take_terminal_damage();
        resized.set_size(16, 2);
        let resized_snapshot = resized
            .terminal_snapshot()
            .expect("resized fake backend should expose terminal snapshots");
        assert_eq!(
            resized_snapshot.lines.len(),
            2,
            "resized snapshot should still match the visible fake backend height"
        );
        let resized_damage = resized.take_terminal_damage();
        assert!(
            matches!(resized_damage, TerminalDamage::Full),
            "resizing should re-arm explicit full redraw damage"
        );

        let legacy = FakeBackend::new(16, 1).render_data();
        let BackendRenderData::Terminal { lines, .. } = legacy;
        assert_eq!(
            lines.len(),
            1,
            "single-row legacy render data should not fabricate extra rows"
        );

        let empty_legacy = FakeBackend::new(16, 0).render_data();
        let BackendRenderData::Terminal {
            lines: empty_lines, ..
        } = empty_legacy;
        assert!(
            empty_lines.is_empty(),
            "zero-height legacy render data should not fabricate extra rows"
        );

        let taller_legacy = FakeBackend::new(16, 8).render_data();
        let BackendRenderData::Terminal {
            lines: tall_lines, ..
        } = taller_legacy;
        assert_eq!(
            tall_lines.len(),
            2,
            "legacy fake render data should stay capped at two placeholder rows"
        );
    }
}
