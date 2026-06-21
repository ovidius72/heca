use super::super::{
    BackendAlert, BackendKeyCode, BackendKeyEvent, BackendModifiers, BackendMouseButton,
    BackendMouseEvent, BackendMouseEventKind, TerminalCell, TerminalLine, TerminalPaletteDefaults,
    TerminalSnapshot, TerminalUnderlineStyle,
};
use crate::backend::{TerminalCursor, TerminalCursorShape};
use std::io::{Result as IoResult, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use wezterm_term::color::{ColorAttribute, ColorPalette};
use wezterm_term::input::{
    KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use wezterm_term::{
    Alert, AlertHandler, CellAttributes, Intensity, Terminal, TerminalConfiguration, TerminalSize,
};
use wezterm_surface::CursorVisibility;

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
}

impl TerminalConfiguration for HecaTerminalConfig {
    fn color_palette(&self) -> ColorPalette {
        self.palette.clone()
    }
}

pub(super) struct TerminalEngine {
    terminal: Terminal,
    cols: usize,
    rows: usize,
    pending_bell: Arc<AtomicBool>,
}

impl TerminalEngine {
    pub(super) fn new(
        cols: usize,
        rows: usize,
        writer: SharedWriter,
        palette_defaults: Option<TerminalPaletteDefaults>,
    ) -> Result<Self, super::PtyError> {
        let mut terminal = Terminal::new(
            terminal_size(cols, rows),
            terminal_config(palette_defaults),
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
        })
    }

    pub(super) fn title(&self) -> &str {
        self.terminal.get_title()
    }

    pub(super) fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
        self.terminal.resize(terminal_size(cols, rows));
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
            cells: vec![blank.clone(); cols],
        };
        let lines = self.visible_lines(cols, rows, &palette, &blank_line);

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
        };
        snapshot.debug_assert_valid();
        snapshot
    }

    fn visible_lines(
        &self,
        cols: usize,
        rows: usize,
        palette: &ColorPalette,
        blank_line: &TerminalLine,
    ) -> Vec<TerminalLine> {
        let screen = self.terminal.screen();
        let visible_count = rows.min(screen.physical_rows.max(1));
        let visible_end = screen.scrollback_rows().max(visible_count);
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

        lines
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
        for slot in cells
            .iter_mut()
            .take(span_end)
            .skip(cell.cell_index())
        {
            *slot = blank_with_attrs(&rendered);
        }
        cells[cell.cell_index()] = rendered;
    }

    TerminalLine { cells }
}

fn terminal_config(palette_defaults: Option<TerminalPaletteDefaults>) -> Arc<HecaTerminalConfig> {
    static DEFAULT_CONFIG: OnceLock<Arc<HecaTerminalConfig>> = OnceLock::new();
    match palette_defaults {
        Some(defaults) => Arc::new(HecaTerminalConfig {
            palette: palette_with_defaults(defaults),
        }),
        None => DEFAULT_CONFIG
            .get_or_init(|| {
                Arc::new(HecaTerminalConfig {
                    palette: ColorPalette::default(),
                })
            })
            .clone(),
    }
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
        wezterm_surface::CursorShape::BlinkingBlock
        | wezterm_surface::CursorShape::SteadyBlock => TerminalCursorShape::Block,
        wezterm_surface::CursorShape::BlinkingUnderline
        | wezterm_surface::CursorShape::SteadyUnderline => TerminalCursorShape::Underline,
        wezterm_surface::CursorShape::BlinkingBar
        | wezterm_surface::CursorShape::SteadyBar => TerminalCursorShape::Bar,
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
        let mut engine = TerminalEngine::new(6, 2, writer, None).expect("engine should initialize");

        engine.advance_bytes(b"\x1b[48;2;30;30;46m\x1b[2J");

        let snapshot = engine.snapshot((8.4, 14.0));
        let expected_bg = [30.0 / 255.0, 30.0 / 255.0, 46.0 / 255.0, 1.0];

        assert!(
            snapshot.lines.iter().all(|line| {
                line.cells
                    .iter()
                    .all(|cell| (cell.bg[0] - expected_bg[0]).abs() < 0.001
                        && (cell.bg[1] - expected_bg[1]).abs() < 0.001
                        && (cell.bg[2] - expected_bg[2]).abs() < 0.001
                        && (cell.bg[3] - expected_bg[3]).abs() < 0.001)
            }),
            "clear-screen background should be preserved across blank cells"
        );
    }

    #[test]
    fn bell_alert_is_captured_once() {
        let writer = SharedWriter::new(Box::new(SinkWriter));
        let mut engine =
            TerminalEngine::new(80, 24, writer, None).expect("terminal engine should initialize");

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
}
