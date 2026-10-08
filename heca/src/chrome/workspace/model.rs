//! What the workspace is handed once a frame: plain owned data, nothing of `AppState`.
//!
//! The rects are **given** — `heca-core`'s scrolling engine placed the columns and the panes in
//! them, scroll animation included — and the workspace only places its children where the model
//! says. A component that computed its own geometry would be taking over the layout engine's job.

use std::collections::HashMap;

use heca_core::layout::{PaneId, Rectangle};

use crate::chrome::column::ColumnShellModel;
use crate::chrome::pane::PaneShellModel;
use crate::chrome::pane_header::PaneHeaderInput;
use crate::chrome::terminal::Terminal;

/// What the columns and the floating panes are, this frame.
pub(crate) struct WorkspaceModel {
    /// The content area, in window coordinates: where the workspace sits, and what clips it.
    pub(crate) area: Rectangle,
    /// The columns, left to right, each holding its panes' models.
    pub(crate) columns: Vec<ColumnShellModel>,
    /// How wide the columns' working area is: a column edge moves a share of it.
    pub(crate) working_width: f32,
    /// The floating panes, back to front.
    pub(crate) floats: Vec<PaneShellModel>,
    /// The places a pane can be put that are open as real space this frame, each with the box the
    /// layout gave it (window coordinates) — empty when none are open.
    pub(crate) places: Vec<heca_core::layout::Place>,
    /// What each pane — tiled or floating — holds and says.
    pub(crate) panes: HashMap<PaneId, PaneEntry>,
    /// **Where a keyboard pick is offering to put the pane**, while one is open: each place for a
    /// new column with its letter, and each column that can be picked. The letters are the
    /// keyboard's, which only the host holds, so they arrive here as data.
    pub(crate) pick: Vec<PickMark>,
}

/// One thing a column pick marks in the workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PickMark {
    /// An empty place for a new column at gap `at`, wearing the letter that picks it.
    Gap { at: usize, letter: char },
    /// An empty place at row `row` of column `col`, wearing the letter that picks it.
    Row {
        col: usize,
        row: usize,
        letter: char,
    },
    /// A column that can be picked. Its letter is the one the hint system offers it, drawn in the
    /// middle of its outline.
    Column(heca_core::layout::ColumnId),
}

/// What one pane is handed besides its frame.
pub(crate) struct PaneEntry {
    /// What runs in the pane: the same terminal every frame, so a rebuilt pane shows the process
    /// that was already running.
    pub(crate) content: Terminal,
    /// What its header shows, when the info bar is on.
    pub(crate) header: Option<PaneHeaderInput>,
}
