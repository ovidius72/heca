use super::TerminalLine;

/// Cursor state for a terminal snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCursorShape {
    Default,
    Block,
    Underline,
    Bar,
}

/// Cursor state for a terminal snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCursor {
    pub col: usize,
    pub row: usize,
    pub visible: bool,
    pub shape: TerminalCursorShape,
}

/// Inclusive-exclusive row range describing invalidated visible terminal rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalRowRange {
    pub start: usize,
    pub end: usize,
}

impl TerminalRowRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Damage metadata for terminal redraw scheduling.
///
/// `Full` means the entire visible viewport should be redrawn. `Rows` scopes
/// the redraw to the provided visible row ranges. `None` means there is no
/// pending visual change to acknowledge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalDamage {
    None,
    Full,
    Rows(Vec<TerminalRowRange>),
}

impl TerminalDamage {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Renderer-agnostic visible terminal state.
///
/// This is intentionally a transitional contract: it gives the renderer enough
/// information to migrate away from the old full-frame `BackendRenderData`
/// transport while keeping terminal semantics out of GPU code.
///
/// Invariants:
/// - `lines.len() == rows`
/// - each line must describe at most `cols` visible cells
/// - `at_bottom` is true iff `viewport_offset == 0`
/// - `scrollback_rows` is the total number of retained content rows
///   (history + visible), so the maximum valid `viewport_offset` is
///   `scrollback_rows.saturating_sub(rows)`
/// - `viewport_top_stable_row` is the wezterm `StableRowIndex` of visible
///   row 0; converting a stable row `s` to a visible row is
///   `s - viewport_top_stable_row` (valid when in `[0, rows)`)
#[derive(Debug, Clone)]
pub struct TerminalSnapshot {
    pub cols: usize,
    pub rows: usize,
    pub cell_w: f32,
    pub cell_h: f32,
    pub default_fg: [f32; 4],
    pub default_bg: [f32; 4],
    pub cursor_color: [f32; 4],
    pub cursor: TerminalCursor,
    pub lines: Vec<TerminalLine>,
    /// Host viewport offset in rows above the live bottom (`0` = pinned to the
    /// live bottom; `N` = `N` rows of history visible below the cursor row).
    /// The backend is the rendering source of truth for this (see
    /// `terminal-01a` Q1); the app mirrors it into the chrome store for
    /// observability.
    pub viewport_offset: usize,
    /// Whether the viewport is pinned to the live bottom (`viewport_offset == 0`).
    pub at_bottom: bool,
    /// Total retained contents rows (history + visible). Scrollbar thumb sizing
    /// and the max viewport offset derive from this.
    pub scrollback_rows: usize,
    /// The wezterm `StableRowIndex` of visible row 0. Host-grid selections are
    /// stored in stable-row coordinates (anchored to content, survive viewport
    /// scroll); rendering and extraction convert via `visible = stable -
    /// viewport_top_stable_row`.
    pub viewport_top_stable_row: isize,
}

impl TerminalSnapshot {
    pub fn debug_assert_valid(&self) {
        debug_assert_eq!(
            self.lines.len(),
            self.rows,
            "terminal snapshot must contain one line per visible row"
        );
        debug_assert!(
            self.lines.iter().all(|line| line.cells.len() <= self.cols),
            "terminal snapshot lines must not exceed visible column count"
        );
        debug_assert_eq!(
            self.at_bottom,
            self.viewport_offset == 0,
            "terminal snapshot `at_bottom` must agree with `viewport_offset`"
        );
        debug_assert!(
            self.viewport_offset <= self.scrollback_rows.saturating_sub(self.rows),
            "terminal snapshot `viewport_offset` must stay within `[0, scrollback_rows - rows]`"
        );
    }
}
