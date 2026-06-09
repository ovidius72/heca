//! Pane content backends — abstraction over terminal, neovim, and browser panes.
//!
//! Currently only `Terminal` is implemented. `Neovim` and `Browser` are
//! planned future backends and should be added here when implemented.

pub mod fake;
pub mod terminal;

pub use fake::FakeBackend;

/// Type of pane backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneType {
    Terminal,
}

/// A single cell in a terminal grid.
#[derive(Debug, Clone)]
pub struct TerminalCell {
    pub c: char,
    pub fg: [f32; 4],
    pub bg: [f32; 4],
    pub bold: bool,
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

    /// Poll for updates (read PTY output, process events, etc.).
    /// Call this every frame before rendering.
    /// Returns true if new data was received and a redraw is needed.
    fn update(&mut self) -> bool;

    /// Get the current renderable content.
    fn render_data(&self) -> BackendRenderData;

    /// Whether the backend has exited and the pane should be closed.
    fn should_close(&self) -> bool;

    /// Logical cell size for this backend, used for mouse→SGR coordinate conversion
    /// and terminal rendering. Returns (cell_w, cell_h) in logical pixels.
    /// Default: (8.4, 14.0) — a reasonable monospace approximation.
    fn cell_size(&self) -> (f32, f32) {
        (8.4, 14.0)
    }
}
