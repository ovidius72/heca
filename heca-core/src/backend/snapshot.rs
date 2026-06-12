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
#[derive(Debug, Clone)]
pub struct TerminalSnapshot {
    pub cols: usize,
    pub rows: usize,
    pub cell_w: f32,
    pub cell_h: f32,
    pub default_fg: [f32; 4],
    pub default_bg: [f32; 4],
    pub cursor: TerminalCursor,
    pub lines: Vec<TerminalLine>,
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
    }
}
