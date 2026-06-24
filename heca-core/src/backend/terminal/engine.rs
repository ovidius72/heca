use super::super::{
    BackendAlert, BackendKeyCode, BackendKeyEvent, BackendModifiers, BackendMouseButton,
    BackendMouseEvent, BackendMouseEventKind, TerminalCell, TerminalLine, TerminalPaletteDefaults,
    TerminalSnapshot, TerminalUnderlineStyle,
};
use crate::backend::{TerminalCursor, TerminalCursorShape};
use std::io::{Result as IoResult, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use wezterm_surface::CursorVisibility;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use wezterm_term::input::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use wezterm_term::{
    Alert, AlertHandler, CellAttributes, Intensity, Terminal, TerminalConfiguration, TerminalSize,
};

/// Small wrapper around a shared PTY writer so `wezterm-term` can encode
/// responses and future keyboard/mouse input directly to the PTY.
#[derive(Clone)]
pub(super) struct SharedWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl SharedWriter {
    pub(super) fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(writer)),
        }
    }

    pub(super) fn write_bytes(&self, buf: &[u8]) -> IoResult<()> {
        let mut writer = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        writer.write_all(buf)?;
        writer.flush()
    }
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let mut writer = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        writer.write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        let mut writer = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        writer.flush()
    }
}

#[derive(Debug)]
struct BellHandler {
    pending_bell: Arc<AtomicBool>,
}

impl AlertHandler for BellHandler {
    fn alert(&mut self, alert: Alert) {
        if matches!(alert, Alert::Bell) {
            self.pending_bell.store(true, Ordering::SeqCst);
        }
    }
}

#[derive(Debug)]
struct HecaTerminalConfig {
    palette: ColorPalette,
    scrollback_size: usize,
}

impl TerminalConfiguration for HecaTerminalConfig {
    fn color_palette(&self) -> ColorPalette {
        self.palette.clone()
    }

    fn scrollback_size(&self) -> usize {
        self.scrollback_size
    }
}

pub(super) struct TerminalEngine {
    terminal: Terminal,
    cols: usize,
    rows: usize,
    pending_bell: Arc<AtomicBool>,
    /// Host viewport offset in rows above the live bottom.
    ///
    /// `0` = pinned to the live bottom (the default, matching the pre-`terminal-01a`
    /// behaviour of always projecting the bottom viewport). `N` = `N` rows of
    /// history are visible below the cursor row. Clamped to
    /// `[0, scrollback_rows - visible_rows]`.
    ///
    /// The engine is the rendering source of truth for this value (Q1 hybrid
    /// ownership); the app mirrors it into the chrome store for observability.
    viewport_offset: usize,
    /// Set whenever `viewport_offset` changes, so the next damage read forces a
    /// full retained-layer re-render with the new viewport rows (Q6: viewport
    /// motion produces `TerminalDamage::Full`; incremental viewport damage is
    /// the separate `terminal-01` phase).
    viewport_changed: bool,
}

impl TerminalEngine {
    pub(super) fn new(
        cols: usize,
        rows: usize,
        writer: SharedWriter,
        palette_defaults: Option<TerminalPaletteDefaults>,
        scrollback_size: usize,
    ) -> Result<Self, super::PtyError> {
        let mut terminal = Terminal::new(
            terminal_size(cols, rows),
            terminal_config(palette_defaults, scrollback_size),
            "heca",
            env!("CARGO_PKG_VERSION"),
            Box::new(writer),
        );
        let pending_bell = Arc::new(AtomicBool::new(false));
        terminal.set_notification_handler(Box::new(BellHandler {
            pending_bell: Arc::clone(&pending_bell),
        }));

        Ok(Self {
            terminal,
            cols,
            rows,
            pending_bell,
            viewport_offset: 0,
            viewport_changed: false,
        })
    }

    pub(super) fn title(&self) -> &str {
        self.terminal.get_title()
    }

    pub(super) fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
        self.terminal.resize(terminal_size(cols, rows));
        // A resize reflows scrollback (visible rows change, history moves), which
        // changes the max valid offset. Re-clamp AFTER the wezterm resize so the
        // boundary reflects the reflowed scrollback, never pointing past the new top.
        let max_offset = self.max_viewport_offset();
        if self.viewport_offset > max_offset {
            self.viewport_offset = max_offset;
            self.viewport_changed = true;
        }
    }

    pub(super) fn advance_bytes(&mut self, bytes: &[u8]) {
        self.terminal.advance_bytes(bytes);
    }

    pub(super) fn take_alerts(&self) -> Vec<BackendAlert> {
        if self.pending_bell.swap(false, Ordering::SeqCst) {
            vec![BackendAlert::Bell]
        } else {
            Vec::new()
        }
    }

    /// Current host viewport offset in rows above the live bottom.
    ///
    /// Test-only inspection helper; production reads the viewport from the
    /// `TerminalSnapshot` (the rendering source of truth per Q1).
    #[cfg(test)]
    pub(super) fn viewport_offset(&self) -> usize {
        self.viewport_offset
    }

    /// Whether the viewport is pinned to the live bottom (`viewport_offset == 0`).
    ///
    /// Test-only inspection helper; production reads `at_bottom` from the
    /// `TerminalSnapshot`.
    #[cfg(test)]
    pub(super) fn at_bottom(&self) -> bool {
        self.viewport_offset == 0
    }

    /// Test-only setter that bypasses the clamp, used to construct a stale stored
    /// offset for `reconcile_viewport_offset` unit tests (the write paths all clamp
    /// on entry, so stale state can only arise from a scrollback shrink).
    #[cfg(test)]
    pub(super) fn set_viewport_offset_for_test(&mut self, offset: usize) {
        self.viewport_offset = offset;
    }

    /// Maximum valid viewport offset = total retained rows minus the visible
    /// row count. Returns `0` when there is no scrollback yet.
    fn max_viewport_offset(&self) -> usize {
        let screen = self.terminal.screen();
        let visible_count = self.rows.min(screen.physical_rows.max(1));
        screen.scrollback_rows().saturating_sub(visible_count)
    }

    /// Scroll the host viewport by `delta_rows`.
    ///
    /// Positive values move toward history (offset increases); negative values
    /// move toward the live bottom (offset decreases). The offset is clamped to
    /// `[0, max_viewport_offset()]`. No-op if the clamped value does not change.
    pub(super) fn scroll_viewport(&mut self, delta_rows: i32) {
        let target = if delta_rows >= 0 {
            self.viewport_offset
                .saturating_add(delta_rows.try_into().unwrap_or(usize::MAX))
        } else {
            self.viewport_offset
                .saturating_sub((-delta_rows).try_into().unwrap_or(usize::MAX))
        };
        let clamped = target.min(self.max_viewport_offset());
        if clamped != self.viewport_offset {
            self.viewport_offset = clamped;
            self.viewport_changed = true;
        }
    }

    /// Jump the viewport to the top of scrollback (maximum offset).
    pub(super) fn scroll_to_top(&mut self) {
        let max_offset = self.max_viewport_offset();
        if self.viewport_offset != max_offset {
            self.viewport_offset = max_offset;
            self.viewport_changed = true;
        }
    }

    /// Snap the viewport to the live bottom (`viewport_offset = 0`).
    pub(super) fn scroll_to_bottom(&mut self) {
        if self.viewport_offset != 0 {
            self.viewport_offset = 0;
            self.viewport_changed = true;
        }
    }

    /// Drain the viewport-changed flag. Returns `true` when the viewport moved
    /// since the last call, which the host must treat as `TerminalDamage::Full`
    /// for the retained terminal layer (Q6).
    pub(super) fn take_viewport_changed(&mut self) -> bool {
        std::mem::take(&mut self.viewport_changed)
    }

    /// Re-clamp the stored `viewport_offset` to the current scrollback boundary and
    /// write the correction back.
    ///
    /// Scrollback can shrink without a resize (alt-screen entry / `\x1b[2J` clears
    /// drop `scrollback_rows` to near zero), which would otherwise leave the stored
    /// offset pointing past the new top. This keeps the engine's stored state
    /// self-consistent — the single source of truth — rather than silently reading a
    /// corrected value in `visible_lines` while the stored field drifts. Any
    /// correction arms `viewport_changed` so it surfaces as `TerminalDamage::Full`
    /// through the damage system.
    ///
    /// Called at the end of [`TerminalBackend::update`] (right after PTY output is
    /// drained, where alt-screen/clear transitions happen), so the stored offset is
    /// consistent before any downstream `snapshot()` read.
    pub(super) fn reconcile_viewport_offset(&mut self) -> bool {
        let max_offset = self.max_viewport_offset();
        if self.viewport_offset > max_offset {
            self.viewport_offset = max_offset;
            self.viewport_changed = true;
            true
        } else {
            false
        }
    }

    pub(super) fn process_key_event(&mut self, event: &BackendKeyEvent) -> bool {
        self.terminal
            .key_down(map_key_code(event.code), map_modifiers(event.modifiers))
            .is_ok()
    }

    pub(super) fn process_mouse_event(&mut self, event: &BackendMouseEvent) -> bool {
        self.terminal
            .mouse_event(MouseEvent {
                kind: map_mouse_event_kind(event.kind),
                x: event.col,
                y: event.row as i64,
                x_pixel_offset: event.x_pixel_offset,
                y_pixel_offset: event.y_pixel_offset,
                button: map_mouse_button(event.button),
                modifiers: map_modifiers(event.modifiers),
            })
            .is_ok()
    }

    pub(super) fn focus_changed(&mut self, focused: bool) {
        self.terminal.focus_changed(focused);
    }

    pub(super) fn snapshot(&self, cell_size: (f32, f32)) -> TerminalSnapshot {
        let (cell_w, cell_h) = cell_size;
        let palette = self.terminal.palette();
        let blank = blank_cell(&palette);
        let size = self.terminal.get_size();
        let cols = size.cols.max(1);
        let rows = size.rows.max(1);
        let blank_line = TerminalLine {
            cells: vec![blank; cols],
        };
        let (lines, scrollback_rows, viewport_offset) =
            self.visible_lines(cols, rows, &palette, &blank_line);

        let cursor = self.terminal.cursor_pos();
        let snapshot = TerminalSnapshot {
            cols,
            rows,
            cell_w,
            cell_h,
            default_fg: to_rgba(palette.resolve_fg(ColorAttribute::Default)),
            default_bg: to_rgba(palette.resolve_bg(ColorAttribute::Default)),
            cursor_color: to_rgba(palette.cursor_bg),
            cursor: TerminalCursor {
                col: cursor.x.min(cols.saturating_sub(1)),
                row: (cursor.y.max(0) as usize).min(rows.saturating_sub(1)),
                visible: cols > 0 && rows > 0 && cursor.visibility == CursorVisibility::Visible,
                shape: map_cursor_shape(cursor.shape),
            },
            lines,
            viewport_offset,
            at_bottom: viewport_offset == 0,
            scrollback_rows,
        };
        snapshot.debug_assert_valid();
        snapshot
    }

    pub(super) fn current_seqno(&self) -> usize {
        self.terminal.current_seqno()
    }

    pub(super) fn visible_top_stable_row(&self) -> isize {
        self.terminal.screen().visible_row_to_stable_row(0)
    }

    pub(super) fn changed_visible_rows_since(&self, seqno: usize) -> Vec<usize> {
        let screen = self.terminal.screen();
        let rows = screen.physical_rows.max(1);
        let top = screen.visible_row_to_stable_row(0);
        let bottom = screen.visible_row_to_stable_row(rows.saturating_sub(1) as i64);
        screen
            .get_changed_stable_rows(top..bottom.saturating_add(1), seqno)
            .into_iter()
            .filter_map(|stable_row| {
                let visible_row = stable_row - top;
                usize::try_from(visible_row).ok().filter(|row| *row < rows)
            })
            .collect()
    }

    fn visible_lines(
        &self,
        cols: usize,
        rows: usize,
        palette: &ColorPalette,
        blank_line: &TerminalLine,
    ) -> (Vec<TerminalLine>, usize, usize) {
        let screen = self.terminal.screen();
        let visible_count = rows.min(screen.physical_rows.max(1));
        let total = screen.scrollback_rows();
        let max_offset = total.saturating_sub(visible_count);
        // Defensive read-clamp: `reconcile_viewport_offset` (called at the end of
        // `update`) keeps the stored offset within bounds, so this normally does
        // nothing. It stays as a belt-and-suspenders guarantee that a snapshot is
        // always visually correct even if a read happens outside the update path.
        let offset = self.viewport_offset.min(max_offset);
        // `total` is the bottom of the live viewport; subtract `offset` to walk
        // `offset` rows up into history, then take `visible_count` rows below.
        let visible_end = total.saturating_sub(offset);
        let visible_start = visible_end.saturating_sub(visible_count);
        let mut lines = Vec::with_capacity(rows);

        for mut line in screen
            .lines_in_phys_range(visible_start..visible_end)
            .into_iter()
            .take(rows)
        {
            lines.push(snapshot_line(&mut line, cols, palette, blank_line));
        }

        while lines.len() < rows {
            lines.push(blank_line.clone());
        }

        (lines, total, offset)
    }
}

fn snapshot_line(
    line: &mut wezterm_term::Line,
    cols: usize,
    palette: &ColorPalette,
    blank_line: &TerminalLine,
) -> TerminalLine {
    let mut cells = blank_line.cells.clone();
    for (idx, cell) in line.cells_mut().iter().enumerate().take(cols) {
        cells[idx] = terminal_cell(" ", 1, cell.attrs(), palette);
    }
    for cell in line.visible_cells() {
        if cell.cell_index() >= cols {
            continue;
        }

        let rendered = terminal_cell(cell.str(), cell.width(), cell.attrs(), palette);
        let span_end = (cell.cell_index() + rendered.width).min(cols);
        for slot in cells.iter_mut().take(span_end).skip(cell.cell_index()) {
            *slot = blank_with_attrs(&rendered);
        }
        cells[cell.cell_index()] = rendered;
    }

    TerminalLine { cells }
}

fn terminal_config(
    palette_defaults: Option<TerminalPaletteDefaults>,
    scrollback_size: usize,
) -> Arc<HecaTerminalConfig> {
    // Engines are created once per pane, so a fresh `Arc` per engine is cheap and
    // keeps the `scrollback_size` override correct (a shared `OnceLock` cache would
    // pin the first-seen size and silently ignore later overrides).
    Arc::new(HecaTerminalConfig {
        palette: match palette_defaults {
            Some(defaults) => palette_with_defaults(defaults),
            None => ColorPalette::default(),
        },
        scrollback_size,
    })
}

fn palette_with_defaults(defaults: TerminalPaletteDefaults) -> ColorPalette {
    let mut palette = ColorPalette::default();
    if let Some(foreground) = defaults.foreground {
        palette.foreground = rgba_u8(foreground);
    }
    if let Some(background) = defaults.background {
        palette.background = rgba_u8(background);
    }
    if let Some(cursor_fg) = defaults.cursor_fg {
        palette.cursor_fg = rgba_u8(cursor_fg);
    }
    if let Some(cursor_bg) = defaults.cursor_bg {
        palette.cursor_bg = rgba_u8(cursor_bg);
    }
    if let Some(cursor_border) = defaults.cursor_border {
        palette.cursor_border = rgba_u8(cursor_border);
    }
    if let Some(selection_fg) = defaults.selection_fg {
        palette.selection_fg = rgba_u8(selection_fg);
    }
    if let Some(selection_bg) = defaults.selection_bg {
        palette.selection_bg = rgba_u8(selection_bg);
    }
    if let Some(ansi) = defaults.ansi {
        for (idx, color) in ansi.into_iter().enumerate() {
            palette.colors.0[idx] = rgba_u8(color);
        }
    }
    if let Some(brights) = defaults.brights {
        for (idx, color) in brights.into_iter().enumerate() {
            palette.colors.0[idx + 8] = rgba_u8(color);
        }
    }
    palette
}

fn rgba_u8(color: [u8; 4]) -> wezterm_term::color::SrgbaTuple {
    wezterm_term::color::SrgbaTuple(
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        color[3] as f32 / 255.0,
    )
}

fn map_key_code(code: BackendKeyCode) -> KeyCode {
    match code {
        BackendKeyCode::Char(ch) => KeyCode::Char(ch),
        BackendKeyCode::Enter => KeyCode::Enter,
        BackendKeyCode::Backspace => KeyCode::Backspace,
        BackendKeyCode::Tab => KeyCode::Tab,
        BackendKeyCode::Escape => KeyCode::Escape,
        BackendKeyCode::LeftArrow => KeyCode::LeftArrow,
        BackendKeyCode::RightArrow => KeyCode::RightArrow,
        BackendKeyCode::UpArrow => KeyCode::UpArrow,
        BackendKeyCode::DownArrow => KeyCode::DownArrow,
        BackendKeyCode::Home => KeyCode::Home,
        BackendKeyCode::End => KeyCode::End,
        BackendKeyCode::PageUp => KeyCode::PageUp,
        BackendKeyCode::PageDown => KeyCode::PageDown,
        BackendKeyCode::Insert => KeyCode::Insert,
        BackendKeyCode::Delete => KeyCode::Delete,
        BackendKeyCode::Function(number) => KeyCode::Function(number),
    }
}

fn map_modifiers(modifiers: BackendModifiers) -> KeyModifiers {
    let mut mapped = KeyModifiers::NONE;
    if modifiers.ctrl {
        mapped |= KeyModifiers::CTRL;
    }
    if modifiers.shift {
        mapped |= KeyModifiers::SHIFT;
    }
    if modifiers.alt {
        mapped |= KeyModifiers::ALT;
    }
    if modifiers.super_ {
        mapped |= KeyModifiers::SUPER;
    }
    mapped
}

fn map_mouse_button(button: BackendMouseButton) -> MouseButton {
    match button {
        BackendMouseButton::Left => MouseButton::Left,
        BackendMouseButton::Middle => MouseButton::Middle,
        BackendMouseButton::Right => MouseButton::Right,
        BackendMouseButton::WheelUp(amount) => MouseButton::WheelUp(amount),
        BackendMouseButton::WheelDown(amount) => MouseButton::WheelDown(amount),
        BackendMouseButton::WheelLeft(amount) => MouseButton::WheelLeft(amount),
        BackendMouseButton::WheelRight(amount) => MouseButton::WheelRight(amount),
        BackendMouseButton::None => MouseButton::None,
    }
}

fn map_mouse_event_kind(kind: BackendMouseEventKind) -> MouseEventKind {
    match kind {
        BackendMouseEventKind::Press => MouseEventKind::Press,
        BackendMouseEventKind::Release => MouseEventKind::Release,
        BackendMouseEventKind::Move => MouseEventKind::Move,
    }
}

fn terminal_size(cols: usize, rows: usize) -> TerminalSize {
    TerminalSize {
        cols: cols.max(1),
        rows: rows.max(1),
        pixel_width: 0,
        pixel_height: 0,
        dpi: 0,
    }
}

fn blank_cell(palette: &ColorPalette) -> TerminalCell {
    TerminalCell {
        text: " ".to_string(),
        fg: to_rgba(palette.resolve_fg(ColorAttribute::Default)),
        bg: to_rgba(palette.resolve_bg(ColorAttribute::Default)),
        bold: false,
        italic: false,
        underline: TerminalUnderlineStyle::None,
        width: 1,
    }
}

fn blank_with_attrs(cell: &TerminalCell) -> TerminalCell {
    TerminalCell {
        text: " ".to_string(),
        fg: cell.fg,
        bg: cell.bg,
        bold: cell.bold,
        italic: cell.italic,
        underline: TerminalUnderlineStyle::None,
        width: 1,
    }
}

fn terminal_cell(
    text: &str,
    width: usize,
    attrs: &CellAttributes,
    palette: &ColorPalette,
) -> TerminalCell {
    let mut fg = to_rgba(resolve_terminal_fg(attrs, palette));
    let mut bg = to_rgba(palette.resolve_bg(attrs.background()));
    if attrs.reverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    if attrs.invisible() {
        fg[3] = 0.0;
    } else if attrs.intensity() == Intensity::Half {
        fg[0] *= 0.7;
        fg[1] *= 0.7;
        fg[2] *= 0.7;
    }

    TerminalCell {
        text: if text.is_empty() {
            " ".to_string()
        } else {
            text.to_string()
        },
        fg,
        bg,
        bold: attrs.intensity() == Intensity::Bold,
        italic: attrs.italic(),
        underline: map_underline_style(attrs.underline()),
        width: width.max(1),
    }
}

fn map_underline_style(underline: wezterm_term::Underline) -> TerminalUnderlineStyle {
    match underline {
        wezterm_term::Underline::None => TerminalUnderlineStyle::None,
        wezterm_term::Underline::Single => TerminalUnderlineStyle::Single,
        wezterm_term::Underline::Double => TerminalUnderlineStyle::Double,
        wezterm_term::Underline::Curly => TerminalUnderlineStyle::Curly,
        wezterm_term::Underline::Dotted => TerminalUnderlineStyle::Dotted,
        wezterm_term::Underline::Dashed => TerminalUnderlineStyle::Dashed,
    }
}

fn resolve_terminal_fg(
    attrs: &CellAttributes,
    palette: &ColorPalette,
) -> wezterm_term::color::SrgbaTuple {
    match attrs.foreground() {
        ColorAttribute::PaletteIndex(idx) if idx < 8 && attrs.intensity() == Intensity::Bold => {
            palette.resolve_fg(ColorAttribute::PaletteIndex(idx + 8))
        }
        fg => palette.resolve_fg(fg),
    }
}

/// Convert wezterm's straight-alpha SRGBA tuple into the renderer's linear-ish
/// float RGBA array without premultiplying alpha.
fn to_rgba(color: wezterm_term::color::SrgbaTuple) -> [f32; 4] {
    let (r, g, b, a) = color.to_tuple_rgba();
    [r, g, b, a]
}

fn map_cursor_shape(shape: wezterm_surface::CursorShape) -> TerminalCursorShape {
    match shape {
        wezterm_surface::CursorShape::Default => TerminalCursorShape::Default,
        wezterm_surface::CursorShape::BlinkingBlock | wezterm_surface::CursorShape::SteadyBlock => {
            TerminalCursorShape::Block
        }
        wezterm_surface::CursorShape::BlinkingUnderline
        | wezterm_surface::CursorShape::SteadyUnderline => TerminalCursorShape::Underline,
        wezterm_surface::CursorShape::BlinkingBar | wezterm_surface::CursorShape::SteadyBar => {
            TerminalCursorShape::Bar
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::TerminalUnderlineStyle;

    struct SinkWriter;

    impl Write for SinkWriter {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> IoResult<()> {
            Ok(())
        }
    }

    #[test]
    fn snapshot_preserves_background_colored_blank_cells_after_clear() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine =
            TerminalEngine::new(6, 2, writer, None, crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE)
                .expect("engine should initialize");

        engine.advance_bytes(b"\x1b[48;2;30;30;46m\x1b[2J");

        let snapshot = engine.snapshot((8.4, 14.0));
        let expected_bg = [30.0 / 255.0, 30.0 / 255.0, 46.0 / 255.0, 1.0];

        assert!(
            snapshot.lines.iter().all(|line| {
                line.cells.iter().all(|cell| {
                    (cell.bg[0] - expected_bg[0]).abs() < 0.001
                        && (cell.bg[1] - expected_bg[1]).abs() < 0.001
                        && (cell.bg[2] - expected_bg[2]).abs() < 0.001
                        && (cell.bg[3] - expected_bg[3]).abs() < 0.001
                })
            }),
            "clear-screen background should be preserved across blank cells"
        );
    }

    #[test]
    fn bell_alert_is_captured_once() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine = TerminalEngine::new(80, 24, writer, None, crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE)
            .expect("terminal engine should initialize");

        engine.advance_bytes(b"\x07");
        assert_eq!(engine.take_alerts(), vec![BackendAlert::Bell]);
        assert!(engine.take_alerts().is_empty());
    }

    #[test]
    fn map_underline_style_preserves_all_wezterm_variants() {
        assert_eq!(
            map_underline_style(wezterm_term::Underline::None),
            TerminalUnderlineStyle::None
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Single),
            TerminalUnderlineStyle::Single
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Double),
            TerminalUnderlineStyle::Double
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Curly),
            TerminalUnderlineStyle::Curly
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Dotted),
            TerminalUnderlineStyle::Dotted
        );
        assert_eq!(
            map_underline_style(wezterm_term::Underline::Dashed),
            TerminalUnderlineStyle::Dashed
        );
    }

    /// Build an engine with enough scrollback to make viewport motion observable.
    fn viewport_engine(cols: usize, rows: usize, scrollback_size: usize) -> TerminalEngine {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        TerminalEngine::new(cols, rows, writer, None, scrollback_size)
            .expect("viewport test engine should initialize")
    }

    /// Fill the scrollback with `count` distinct printable lines so the viewport
    /// has real history to scroll into. Each line is `cols` cells wide.
    fn fill_scrollback(engine: &mut TerminalEngine, cols: usize, count: usize) {
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
    fn max_offset(engine: &TerminalEngine) -> usize {
        engine.max_viewport_offset()
    }

    #[test]
    fn viewport_starts_pinned_to_bottom_with_zero_offset() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);

        assert_eq!(engine.viewport_offset(), 0, "fresh engine pins to live bottom");
        assert!(engine.at_bottom(), "at_bottom is true at offset 0");
        assert!(!engine.take_viewport_changed(), "no motion yet ⇒ no viewport damage");

        let snapshot = engine.snapshot((8.0, 14.0));
        assert_eq!(snapshot.viewport_offset, 0);
        assert!(snapshot.at_bottom);
        assert!(
            snapshot.scrollback_rows > 4,
            "scrollback_rows should retain history above the visible rows"
        );
        assert_eq!(
            snapshot.scrollback_rows.saturating_sub(4),
            max_offset(&engine),
            "snapshot scrollback_rows minus visible equals the max offset"
        );
    }

    #[test]
    fn scroll_viewport_clamps_to_max_offset_and_sets_damage_flag() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        let max_offset = max_offset(&engine);
        assert!(max_offset > 0, "fixture should produce some scrollback history");

        engine.scroll_viewport(3);
        assert_eq!(engine.viewport_offset(), 3);
        assert!(!engine.at_bottom());
        assert!(engine.take_viewport_changed(), "motion sets the viewport-changed flag");
        assert!(!engine.take_viewport_changed(), "flag drains once");

        // Overscroll clamps to max_offset.
        engine.scroll_viewport(100);
        assert_eq!(engine.viewport_offset(), max_offset);
        assert!(engine.take_viewport_changed());

        // Scrolling back down clamps at bottom (offset 0).
        engine.scroll_viewport(-100);
        assert_eq!(engine.viewport_offset(), 0);
        assert!(engine.at_bottom());
        assert!(engine.take_viewport_changed());

        // No-op scrolls (already at boundary) do not arm damage.
        engine.scroll_viewport(-1);
        assert_eq!(engine.viewport_offset(), 0);
        assert!(!engine.take_viewport_changed(), "clamped no-op must not arm damage");
    }

    #[test]
    fn scroll_to_top_and_bottom_jump_the_viewport() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        let max_offset = max_offset(&engine);
        assert!(max_offset > 0);

        engine.scroll_to_top();
        assert_eq!(engine.viewport_offset(), max_offset);
        assert!(engine.take_viewport_changed());

        engine.scroll_to_top();
        assert!(!engine.take_viewport_changed(), "already-at-top no-op must not arm damage");

        engine.scroll_to_bottom();
        assert_eq!(engine.viewport_offset(), 0);
        assert!(engine.at_bottom());
        assert!(engine.take_viewport_changed());

        engine.scroll_to_bottom();
        assert!(!engine.take_viewport_changed(), "already-at-bottom no-op must not arm damage");
    }

    #[test]
    fn snapshot_projects_history_rows_when_viewport_is_scrolled() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        // Scrolling up 4 rows should reveal history rows instead of the live bottom.
        engine.scroll_viewport(4);
        let snapshot = engine.snapshot((8.0, 14.0));
        assert_eq!(snapshot.viewport_offset, 4);
        assert!(!snapshot.at_bottom);
        assert_eq!(snapshot.lines.len(), 4, "snapshot still reports exactly `rows` lines");
        snapshot.debug_assert_valid();
    }

    #[test]
    fn resize_re_clamps_viewport_offset_when_scrollback_shrinks() {
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        let total = engine.snapshot((8.0, 14.0)).scrollback_rows;
        engine.scroll_to_top();
        assert_eq!(engine.viewport_offset(), total.saturating_sub(4));
        let _ = engine.take_viewport_changed();

        // Grow visible rows so max_offset shrinks; the stored offset must clamp.
        engine.resize(20, 8);
        assert_eq!(
            engine.viewport_offset(),
            total.saturating_sub(8),
            "resize must re-clamp offset to new max (total - visible 8)"
        );
        assert!(engine.take_viewport_changed(), "re-clamp on resize arms damage");
    }

    #[test]
    fn scrollback_size_override_replaces_default_capacity() {
        // Drive the engine with a non-default scrollback size and confirm it is
        // plumbed through `HecaTerminalConfig` by filling past the wezterm default
        // (3500) up to the override. Total retained rows should be bounded by the
        // formula `scrollback_size + visible_rows` (the override caps history).
        let scrollback_size = 50;
        let rows = 2;
        let mut engine = viewport_engine(10, rows, scrollback_size);
        fill_scrollback(&mut engine, 10, 80);
        let snapshot = engine.snapshot((8.0, 14.0));
        assert_eq!(snapshot.rows, rows);
        assert_eq!(snapshot.lines.len(), rows);
        assert!(
            snapshot.scrollback_rows <= scrollback_size + rows,
            "scrollback_size override should cap retained history at scrollback_size + visible_rows"
        );
    }

    #[test]
    fn reconcile_viewport_offset_snaps_to_bottom_when_scrollback_shrinks() {
        // The drift fix targets the case where scrollback shrinks without a resize
        // (alt-screen entry / clear dropping `scrollback_rows`), leaving the stored
        // offset pointing past the new top. `scroll_viewport`/`scroll_to_top`/resize
        // all clamp on write, so the only way to construct a stale stored offset is
        // to set it directly — this exercises `reconcile_viewport_offset` as a unit,
        // decoupled from wezterm escape-sequence semantics.
        let mut engine = viewport_engine(20, 4, 3500);
        fill_scrollback(&mut engine, 20, 10);
        let top = max_offset(&engine);
        assert!(top > 0, "fixture should produce scrollback history");

        // Force the stored offset past the boundary (simulating a shrink that
        // hasn't been reconciled yet).
        engine.set_viewport_offset_for_test(top + 5);
        assert_eq!(engine.viewport_offset(), top + 5);
        let _ = engine.take_viewport_changed();

        let changed = engine.reconcile_viewport_offset();
        assert_eq!(
            engine.viewport_offset(),
            top,
            "reconcile must write the stored offset back to the current max"
        );
        assert!(
            changed,
            "reconcile must report a correction when the stored offset was stale"
        );
        assert!(engine.take_viewport_changed(), "correction arms viewport damage");

        // A second reconcile on already-consistent state is a no-op.
        assert!(!engine.reconcile_viewport_offset());
        assert!(!engine.take_viewport_changed());

        // When scrollback has fully collapsed (max_offset == 0), reconcile snaps to
        // the live bottom — the alt-screen case.
        engine.set_viewport_offset_for_test(3);
        // Collapse max_offset to 0 by clearing scrollback via the alt screen.
        engine.advance_bytes(b"\x1b[?1049h");
        let collapsed = max_offset(&engine);
        if collapsed == 0 {
            assert!(engine.reconcile_viewport_offset(), "alt-screen shrink must correct");
            assert_eq!(engine.viewport_offset(), 0, "reconcile snaps to bottom on alt screen");
            assert!(engine.take_viewport_changed());
        }
        // Leave the alt screen so the shared default-config cache / other tests are unaffected.
        engine.advance_bytes(b"\x1b[?1049l");
    }
}
