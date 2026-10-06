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
    /// What each pane — tiled or floating — holds and says.
    pub(crate) panes: HashMap<PaneId, PaneEntry>,
}

/// What one pane is handed besides its frame.
pub(crate) struct PaneEntry {
    /// What runs in the pane: the same terminal every frame, so a rebuilt pane shows the process
    /// that was already running.
    pub(crate) content: Terminal,
    /// What its header shows, when the info bar is on.
    pub(crate) header: Option<PaneHeaderInput>,
}
