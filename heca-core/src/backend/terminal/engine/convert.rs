//! **Converting between heca's terminal types and the terminal library's** — config, colours,
//! cells, keys and the mouse.

use super::*;

pub(super) fn terminal_config(
    palette_defaults: Option<TerminalPaletteDefaults>,
    scrollback_size: &Arc<AtomicUsize>,
) -> Arc<HecaTerminalConfig> {
    // Engines are created once per pane, so a fresh `Arc` per engine is cheap and
    // keeps the `scrollback_size` override correct (a shared `OnceLock` cache would
    // pin the first-seen size and silently ignore later overrides).
    Arc::new(HecaTerminalConfig {
        palette: match palette_defaults {
            Some(defaults) => palette_with_defaults(defaults),
            None => ColorPalette::default(),
        },
        scrollback_size: Arc::clone(scrollback_size),
    })
}

pub(super) fn palette_with_defaults(defaults: TerminalPaletteDefaults) -> ColorPalette {
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

pub(super) fn rgba_u8(color: [u8; 4]) -> wezterm_term::color::SrgbaTuple {
    wezterm_term::color::SrgbaTuple(
        color[0] as f32 / 255.0,
        color[1] as f32 / 255.0,
        color[2] as f32 / 255.0,
        color[3] as f32 / 255.0,
    )
}

pub(super) fn map_key_code(code: BackendKeyCode) -> KeyCode {
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

pub(super) fn map_modifiers(modifiers: BackendModifiers) -> KeyModifiers {
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

pub(super) fn map_mouse_button(button: BackendMouseButton) -> MouseButton {
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

pub(super) fn map_mouse_event_kind(kind: BackendMouseEventKind) -> MouseEventKind {
    match kind {
        BackendMouseEventKind::Press => MouseEventKind::Press,
        BackendMouseEventKind::Release => MouseEventKind::Release,
        BackendMouseEventKind::Move => MouseEventKind::Move,
    }
}

pub(super) fn terminal_size(cols: usize, rows: usize, cell_px: (f32, f32)) -> TerminalSize {
    let cols = cols.max(1);
    let rows = rows.max(1);
    // wezterm derives the per-cell pixel size as `pixel_width / cols`, so keep at
    // least one pixel per cell: a zero would mis-size inline images and divide by
    // zero on Sixel attachment.
    let cell_w = cell_px.0.round().max(1.0) as usize;
    let cell_h = cell_px.1.round().max(1.0) as usize;
    TerminalSize {
        cols,
        rows,
        pixel_width: cols * cell_w,
        pixel_height: rows * cell_h,
        dpi: 96,
    }
}

pub(super) fn blank_cell(palette: &ColorPalette) -> TerminalCell {
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

pub(super) fn blank_with_attrs(cell: &TerminalCell) -> TerminalCell {
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

pub(super) fn terminal_cell(
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

pub(super) fn map_underline_style(underline: wezterm_term::Underline) -> TerminalUnderlineStyle {
    match underline {
        wezterm_term::Underline::None => TerminalUnderlineStyle::None,
        wezterm_term::Underline::Single => TerminalUnderlineStyle::Single,
        wezterm_term::Underline::Double => TerminalUnderlineStyle::Double,
        wezterm_term::Underline::Curly => TerminalUnderlineStyle::Curly,
        wezterm_term::Underline::Dotted => TerminalUnderlineStyle::Dotted,
        wezterm_term::Underline::Dashed => TerminalUnderlineStyle::Dashed,
    }
}

pub(super) fn resolve_terminal_fg(
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
pub(super) fn to_rgba(color: wezterm_term::color::SrgbaTuple) -> [f32; 4] {
    let (r, g, b, a) = color.to_tuple_rgba();
    [r, g, b, a]
}

pub(super) fn map_cursor_shape(shape: wezterm_surface::CursorShape) -> TerminalCursorShape {
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
