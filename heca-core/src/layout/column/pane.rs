//! A pane within a column: what it is.

use crate::layout::types::*;
use crate::runtime::{PaneClosePolicy, PaneRuntime};

/// A pane within a column.
///
/// This is heca's equivalent of NIRI's `Tile<W>` — it wraps the actual content
/// (terminal, neovim, browser) and tracks its layout state.
#[derive(Debug, Clone)]
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    /// User-set display name (from rename). `Some` **overrides** the process-derived
    /// title everywhere it's shown; `None` means the name tracks the running process.
    pub custom_name: Option<String>,
    pub runtime: PaneRuntime,
    pub close_policy: PaneClosePolicy,
    /// The share of the room its column gives its panes that this pane was dragged to, in
    /// `(0, 1]`; `None` = auto. A share, not pixels, so it keeps its proportion at any window size.
    pub height_share: Option<f64>,
    /// Offset applied during interactive move Starting phase (rubberband).
    /// Cleared on transition to Moving. Not used by entry/exit animations.
    pub interactive_move_offset: Point,
}

impl Pane {
    pub fn new(id: PaneId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            custom_name: None,
            runtime: PaneRuntime::default(),
            close_policy: PaneClosePolicy::default(),
            height_share: None,
            interactive_move_offset: Point::default(),
        }
    }
}
