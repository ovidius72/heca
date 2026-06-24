//! Pane content backends — abstraction over terminal, neovim, and browser panes.
//!
//! Currently only `Terminal` is implemented. `Neovim` and `Browser` are
//! planned future backends and should be added here when implemented.

pub mod fake;
pub mod snapshot;
pub mod terminal;

pub use fake::FakeBackend;
pub use snapshot::{
    TerminalCursor, TerminalCursorShape, TerminalDamage, TerminalRowRange, TerminalSnapshot,
};
pub use terminal::{PtyError, ShellIntegrationAssets, TerminalBackend, TerminalBackendOptions};

use crate::runtime::PaneRuntime;

/// Theme-derived default colors for terminal emulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalPaletteDefaults {
    pub foreground: Option<[u8; 4]>,
    pub background: Option<[u8; 4]>,
    pub cursor_fg: Option<[u8; 4]>,
    pub cursor_bg: Option<[u8; 4]>,
    pub cursor_border: Option<[u8; 4]>,
    pub selection_fg: Option<[u8; 4]>,
    pub selection_bg: Option<[u8; 4]>,
    pub ansi: Option<[[u8; 4]; 8]>,
    pub brights: Option<[[u8; 4]; 8]>,
}

/// Type of pane backend.
///
/// Currently only `Terminal` is implemented. Future backends (Neovim, Browser)
/// will add variants here. The `PaneBackend::pane_type()` trait method returns this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneType {
    Terminal,
}

/// Backend-side alerts emitted by pane content sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendAlert {
    Bell,
}

/// Backend-agnostic keyboard modifiers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackendModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_: bool,
}

/// Backend-agnostic key identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKeyCode {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Escape,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Function(u8),
}

/// Structured keyboard event routed to a pane backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendKeyEvent {
    pub code: BackendKeyCode,
    pub modifiers: BackendModifiers,
}

/// Backend-agnostic mouse buttons and wheel steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendMouseButton {
    Left,
    Middle,
    Right,
    WheelUp(usize),
    WheelDown(usize),
    WheelLeft(usize),
    WheelRight(usize),
    None,
}

/// Structured mouse event kind routed to a pane backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendMouseEventKind {
    Press,
    Release,
    Move,
}

/// Structured mouse event routed to a pane backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendMouseEvent {
    pub kind: BackendMouseEventKind,
    pub col: usize,
    pub row: usize,
    pub x_pixel_offset: isize,
    pub y_pixel_offset: isize,
    pub button: BackendMouseButton,
    pub modifiers: BackendModifiers,
}

/// Underline decoration style for a terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalUnderlineStyle {
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

impl TerminalUnderlineStyle {
    pub fn is_visible(self) -> bool {
        self != Self::None
    }
}

/// A single cell in a terminal grid.
#[derive(Debug, Clone)]
pub struct TerminalCell {
    /// Full grapheme/text content for the visible cell anchor.
    pub text: String,
    pub fg: [f32; 4],
    pub bg: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    pub underline: TerminalUnderlineStyle,
    /// Display width in terminal cells for this grapheme cluster.
    pub width: usize,
}

/// A line of terminal cells.
#[derive(Debug, Clone)]
pub struct TerminalLine {
    pub cells: Vec<TerminalCell>,
}

/// Render data produced by a pane backend.
/// The main app iterates this and queues draw commands.
#[derive(Debug, Clone)]
pub enum BackendRenderData {
    Terminal {
        lines: Vec<TerminalLine>,
        cursor_col: usize,
        cursor_row: usize,
        /// Logical cell width for coordinate conversion.
        cell_w: f32,
        /// Logical cell height for coordinate conversion.
        cell_h: f32,
    },
}

/// Trait for pane content backends.
///
/// Each pane (terminal, neovim, browser) implements this trait.
/// The layout engine is agnostic to the backend type.
pub trait PaneBackend: Send {
    /// What kind of pane this is.
    fn pane_type(&self) -> PaneType;

    /// Current title (shown in tab bar / status bar).
    fn title(&self) -> &str;

    /// Resize the backend to the given grid dimensions.
    fn set_size(&mut self, cols: usize, rows: usize);

    /// Send input bytes to the backend (keyboard, mouse, etc.).
    fn process_input(&mut self, data: &[u8]);

    /// Send a structured key event to the backend.
    ///
    /// Returns `true` when the backend handled the event directly. Callers may
    /// fall back to raw byte forwarding when this returns `false`.
    fn process_key_event(&mut self, _event: &BackendKeyEvent) -> bool {
        false
    }

    /// Send a structured mouse event to the backend.
    ///
    /// Returns `true` when the backend handled the event directly.
    fn process_mouse_event(&mut self, _event: &BackendMouseEvent) -> bool {
        false
    }

    /// Notify the backend that pane focus changed.
    fn focus_changed(&mut self, _focused: bool) {}

    /// Drain one-shot backend alerts that were raised since the previous poll.
    fn take_alerts(&mut self) -> Vec<BackendAlert> {
        Vec::new()
    }

    /// Update logical terminal cell metrics used by snapshot rendering.
    ///
    /// This does not necessarily change the PTY grid size by itself; it updates
    /// how the backend reports cell geometry to the renderer.
    fn set_cell_size(&mut self, _cell_w: f32, _cell_h: f32) {}

    /// Poll for updates (read PTY output, process events, etc.).
    /// Call this every frame before rendering.
    /// Returns true if new data was received and a redraw is needed.
    fn update(&mut self) -> bool;

    /// Get a terminal-specific snapshot of the currently visible state.
    ///
    /// This is the incremental migration path away from the legacy
    /// `render_data()` full-copy transport. Terminal backends should return
    /// `Some(snapshot)` and non-terminal backends can use the default `None`.
    ///
    /// The snapshot intentionally stays renderer-agnostic: it carries logical
    /// cell state and cursor state, but no GPU resources.
    fn terminal_snapshot(&self) -> Option<TerminalSnapshot> {
        None
    }

    /// Acknowledge terminal damage after a consumer has finished using the
    /// current terminal state for rendering.
    ///
    /// Unlike `terminal_snapshot()`, this method is explicitly stateful. It is
    /// the only API that should clear pending terminal damage. Backends that
    /// expose `terminal_snapshot()` but do not yet implement incremental damage
    /// reporting should conservatively return `TerminalDamage::Full`.
    fn take_terminal_damage(&mut self) -> TerminalDamage {
        if self.terminal_snapshot().is_some() {
            TerminalDamage::Full
        } else {
            TerminalDamage::None
        }
    }

    /// Get the current renderable content.
    ///
    /// Legacy renderer contract. New terminal work should prefer
    /// `terminal_snapshot()` and dedicated terminal rendering paths.
    fn render_data(&self) -> BackendRenderData;

    /// Whether the backend has exited and the pane should be closed.
    fn should_close(&self) -> bool;

    /// Logical cell size for this backend, used for mouse→SGR coordinate conversion
    /// and terminal rendering. Returns (cell_w, cell_h) in logical pixels.
    /// Default: (8.4, 14.0) — a reasonable monospace approximation.
    fn cell_size(&self) -> (f32, f32) {
        (8.4, 14.0)
    }

    /// Canonical per-pane runtime metadata (program, status, cwd, exit code, git,
    /// content-kind). The backend owns detection (PTY/OS); the app's chrome store
    /// mirrors this reactively. Default: `PaneRuntime::default()` for backends
    /// without detection (e.g. non-terminal/fake backends).
    fn runtime(&self) -> PaneRuntime {
        PaneRuntime::default()
    }

    /// Drain a one-shot exit code captured since the last call (set when the
    /// backend's child process exited). The per-wake monitor drains this to emit
    /// `pane.exited{code}`. Returns `Some(code)` once, then `None` until another
    /// exit is captured. Default: `None` (no exit to report).
    fn take_exit_code(&mut self) -> Option<i32> {
        None
    }

    /// Scroll the host terminal viewport by `delta_rows`.
    ///
    /// Positive values scroll toward history (the viewport moves up, revealing
    /// older rows); negative values scroll toward the live bottom. The offset is
    /// clamped to `[0, scrollback_rows - visible_rows]`. A non-zero offset means
    /// the pane is no longer pinned to the live bottom.
    ///
    /// Only terminal backends with a host scrollback viewport implement this
    /// (see `terminal-01a`). The default is a no-op for backends without host
    /// scrollback (e.g. `FakeBackend`).
    fn scroll_viewport(&mut self, _delta_rows: i32) {}

    /// Jump the host terminal viewport to the top of scrollback (maximum offset).
    /// Default: no-op.
    fn scroll_to_top(&mut self) {}

    /// Snap the host terminal viewport to the live bottom (`viewport_offset = 0`).
    /// Default: no-op.
    fn scroll_to_bottom(&mut self) {}

    /// Fetch the inclusive stable-row range `[start, end]` as renderer-ready
    /// terminal lines, each capped to `cols` cells.
    ///
    /// Host-grid selections are stored in stable-row coordinates (anchored to
    /// content, surviving viewport scroll), so copying a selection that spans
    /// history must fetch content by stable row rather than index the visible
    /// snapshot. Only terminal backends with a host scrollback viewport implement
    /// this; the default returns an empty vec for backends without scrollback.
    fn lines_in_stable_range(
        &self,
        _start: isize,
        _end: isize,
        _cols: usize,
    ) -> Vec<TerminalLine> {
        Vec::new()
    }
}
