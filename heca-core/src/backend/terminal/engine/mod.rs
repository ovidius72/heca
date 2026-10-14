use super::super::{
    BackendAlert, BackendKeyCode, BackendKeyEvent, BackendModifiers, BackendMouseButton,
    BackendMouseEvent, BackendMouseEventKind, GraphicsPlacement, HyperlinkSpan, SearchMatch,
    TerminalCell, TerminalImage, TerminalImageFrame, TerminalLine, TerminalPaletteDefaults,
    TerminalSnapshot, TerminalUnderlineStyle,
};
use crate::backend::{TerminalCursor, TerminalCursorShape};
use crate::layout::animation::{Animation, AnimationConfig};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Result as IoResult, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wezterm_surface::CursorVisibility;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use wezterm_term::input::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use wezterm_term::{
    Alert, AlertHandler, CellAttributes, Clipboard, ClipboardSelection, Intensity, Terminal,
    TerminalConfiguration, TerminalSize,
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

/// Routes `OSC 52` clipboard writes from the terminal program into a shared
/// queue the app drains and forwards to the system clipboard.
///
/// Only **writes** are honored: a program can set the clipboard, but clipboard
/// *reads* over `OSC 52` (the `?` query form) are intentionally unsupported —
/// letting arbitrary terminal output exfiltrate clipboard contents is a known
/// security risk. wezterm parses and base64-decodes the sequence for us; we just
/// capture the decoded text.
struct OscClipboard {
    pending: Arc<Mutex<Vec<String>>>,
}

impl Clipboard for OscClipboard {
    fn set_contents(
        &self,
        _selection: ClipboardSelection,
        data: Option<String>,
    ) -> anyhow::Result<()> {
        if let Some(text) = data
            && !text.is_empty()
        {
            self.pending.lock().unwrap().push(text);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct HecaTerminalConfig {
    palette: ColorPalette,
    /// How many rows of history to keep. Shared with every config this terminal is given: the
    /// terminal library holds on to the config each screen was made with, so a size handed over
    /// in a new config would never reach a running terminal — one cell read live does.
    scrollback_size: Arc<AtomicUsize>,
}

impl TerminalConfiguration for HecaTerminalConfig {
    fn color_palette(&self) -> ColorPalette {
        self.palette.clone()
    }

    fn scrollback_size(&self) -> usize {
        self.scrollback_size.load(Ordering::Relaxed)
    }

    /// Capture Kitty graphics in addition to the Sixel + iTerm2 `OSC 1337`
    /// protocols wezterm parses by default. All three attach `ImageCell`s to the
    /// grid, so enabling this widens inline-image support to every common tool
    /// (`kitten icat`, Yazi's kitty previewer, …) without a protocol-specific
    /// path. See `terminal-09`.
    fn enable_kitty_graphics(&self) -> bool {
        true
    }
}

pub(super) struct TerminalEngine {
    terminal: Terminal,
    /// How many rows of history to keep — the one cell every config of this terminal reads.
    scrollback_size: Arc<AtomicUsize>,
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
    /// Optional animation for smooth viewport scrolling (slice 5). Discrete jumps
    /// (page/line/top/bottom) create an Animation; the wheel sets offset directly.
    /// `advance_animation()` snaps the animated f64 to the nearest integer row
    /// and updates `viewport_offset` when the rounded value changes. Re-targeted
    /// (never queued) on consecutive discrete jumps.
    viewport_anim: Option<Animation>,
    /// Config used to build viewport animations. Defaults to `AnimationConfig::default()`
    /// (the same 250ms `ease_out_cubic` `ViewOffset`/`activate_column` use) so terminal
    /// scroll feels like column scroll. Overridable per-engine (tests use 0ms for
    /// instant completion, or a long duration to exercise the ongoing path) without
    /// any real-time `sleep`.
    viewport_anim_config: AnimationConfig,
    /// Global on/off switch for backend-side viewport easing. When false, the
    /// animated APIs degrade to the immediate paths.
    viewport_anim_enabled: bool,
    /// Detect plain-text URLs in the visible grid and emit them as hyperlink
    /// spans (in addition to explicit OSC 8 links). Default `true`.
    link_detection: bool,
    /// Capture inline-image placements into the snapshot. Default `true`; the app
    /// turns it off via the `terminal.images` config toggle. See `terminal-09`.
    image_capture: bool,
    /// Decoded-image cache keyed by wezterm's source content hash. Each unique
    /// inline image is decoded to RGBA exactly once and shared (`Arc`) across
    /// every snapshot that references it; `None` caches a decode failure so a
    /// broken payload is not retried each frame. `RefCell` because snapshotting
    /// is `&self`. See `terminal-09`.
    decoded_images: RefCell<HashMap<[u8; 32], Option<Arc<TerminalImage>>>>,
    /// Physical cell pixel size `(width, height)` reported to wezterm so it can
    /// size inline images and answer pixel-size queries. See [`Self::set_cell_px`].
    cell_px: (f32, f32),
    /// `OSC 52` clipboard-write requests captured from terminal output, drained
    /// by the app and pushed to the system clipboard. See [`OscClipboard`].
    clipboard_writes: Arc<Mutex<Vec<String>>>,
}

impl TerminalEngine {
    pub(super) fn new(
        cols: usize,
        rows: usize,
        cell_px: (f32, f32),
        writer: SharedWriter,
        palette_defaults: Option<TerminalPaletteDefaults>,
        scrollback_size: usize,
    ) -> Result<Self, super::PtyError> {
        let scrollback_size = Arc::new(AtomicUsize::new(scrollback_size));
        let mut terminal = Terminal::new(
            terminal_size(cols, rows, cell_px),
            terminal_config(palette_defaults, &scrollback_size),
            "heca",
            env!("CARGO_PKG_VERSION"),
            Box::new(writer),
        );
        let pending_bell = Arc::new(AtomicBool::new(false));
        terminal.set_notification_handler(Box::new(BellHandler {
            pending_bell: Arc::clone(&pending_bell),
        }));
        let clipboard_writes = Arc::new(Mutex::new(Vec::new()));
        terminal.set_clipboard(
            &(Arc::new(OscClipboard {
                pending: Arc::clone(&clipboard_writes),
            }) as Arc<dyn Clipboard>),
        );

        Ok(Self {
            terminal,
            cols,
            rows,
            pending_bell,
            scrollback_size,
            viewport_offset: 0,
            viewport_changed: false,
            viewport_anim: None,
            viewport_anim_config: AnimationConfig::default(),
            viewport_anim_enabled: true,
            link_detection: true,
            image_capture: true,
            decoded_images: RefCell::new(HashMap::new()),
            cell_px,
            clipboard_writes,
        })
    }

    /// Enable or disable inline-image capture (the `terminal.images` config toggle).
    pub(super) fn set_image_capture(&mut self, enabled: bool) {
        self.image_capture = enabled;
    }

    /// Update the physical cell pixel size reported to the emulation layer.
    ///
    /// wezterm divides the terminal's pixel dimensions by the cell grid to size
    /// inline images (Sixel/iTerm2/Kitty) and to answer pixel-size queries
    /// (`CSI 14 t` / `CSI 16 t`) that image tools use to scale previews. A stale
    /// or zero size leaves images mis-sized (and divides by zero on Sixel), so we
    /// re-issue the terminal size whenever the renderer's cell metrics change.
    pub(super) fn set_cell_px(&mut self, cell_px: (f32, f32)) {
        if self.cell_px == cell_px {
            return;
        }
        self.cell_px = cell_px;
        self.terminal
            .resize(terminal_size(self.cols, self.rows, cell_px));
    }

    /// Enable or disable plain-text URL auto-detection (linkify).
    pub(super) fn set_link_detection(&mut self, enabled: bool) {
        self.link_detection = enabled;
    }

    pub(super) fn title(&self) -> &str {
        self.terminal.get_title()
    }

    pub(super) fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
        self.terminal
            .resize(terminal_size(cols, rows, self.cell_px));
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

    /// Drain `OSC 52` clipboard-write requests captured since the last poll. The
    /// app forwards them to the system clipboard.
    pub(super) fn take_clipboard_writes(&self) -> Vec<String> {
        std::mem::take(&mut self.clipboard_writes.lock().unwrap())
    }

    /// Whether the program has enabled bracketed paste (DECSET 2004), so pasted
    /// text must be wrapped in `ESC[200~ … ESC[201~`.
    pub(super) fn bracketed_paste_enabled(&self) -> bool {
        self.terminal.bracketed_paste_enabled()
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

}

mod convert;
mod images;
mod links;
mod read;
mod viewport;
use convert::*;
use images::*;
use links::*;

#[cfg(test)]
mod tests;
