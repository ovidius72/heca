//! Tests for the terminal engine, split by what each part does.

use super::*;
use crate::backend::TerminalUnderlineStyle;

mod config;
mod images;
mod links;
mod viewport;

pub(super) struct SinkWriter;

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

/// Palette defaults with only the background set — the rest inherit wezterm's default.
pub(super) fn bg_defaults(bg: [u8; 4]) -> TerminalPaletteDefaults {
    TerminalPaletteDefaults {
        foreground: None,
        background: Some(bg),
        cursor_fg: None,
        cursor_bg: None,
        cursor_border: None,
        selection_fg: None,
        selection_bg: None,
        ansi: None,
        brights: None,
    }
}

/// Build an engine with enough scrollback to make viewport motion observable.
pub(super) fn viewport_engine(cols: usize, rows: usize, scrollback_size: usize) -> TerminalEngine {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    TerminalEngine::new(cols, rows, (8.0, 16.0), writer, None, scrollback_size)
        .expect("viewport test engine should initialize")
}

/// Fill the scrollback with `count` distinct printable lines so the viewport
/// has real history to scroll into. Each line is `cols` cells wide.
pub(super) fn fill_scrollback(engine: &mut TerminalEngine, cols: usize, count: usize) {
    for i in 0..count {
        // Print a marker followed by spaces, then a newline so each line lands
        // in scrollback as the next output row scrolls up.
        let marker = format!("L{i:03}");
        let mut line = marker.into_bytes();
        line.truncate(cols);
        while line.len() < cols {
            line.push(b' ');
        }
        engine.advance_bytes(&line);
        engine.advance_bytes(b"\r\n");
    }
}

/// Read the current max valid viewport offset directly from the engine's own
/// clamp logic (`scrollback_rows - visible_rows`). Deriving the expected
/// boundary from the live engine avoids hardcoding fragile row counts (the
/// exact retained-row total depends on wezterm's scroll timing).
pub(super) fn max_offset(engine: &TerminalEngine) -> usize {
    engine.max_viewport_offset()
}
