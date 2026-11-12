//! What each pane's header needs to know, gathered from the session once a frame.

use super::*;

/// What one pane's header needs to know, as plain data gathered from the session.
pub(crate) struct PaneHeaderInput {
    pub(super) pane_id: PaneId,
    pub(super) ws_idx: usize,
    pub(super) col_idx: usize,
    pub(super) name: String,
    pub(super) custom_name: Option<String>,
    pub(super) runtime: Option<crate::chrome::PanePresentation>,
    pub(super) zoomed: bool,
    pub(super) floating: bool,
}

#[cfg(test)]
impl PaneHeaderInput {
    /// Facts for pane `pane_id` and nothing else — what a test hands a pane.
    pub(crate) fn for_test(pane_id: PaneId) -> Self {
        Self {
            pane_id,
            ws_idx: 0,
            col_idx: 0,
            name: String::new(),
            custom_name: None,
            runtime: None,
            zoomed: false,
            floating: false,
        }
    }

    /// Which pane these facts are about.
    pub(crate) fn pane(&self) -> PaneId {
        self.pane_id
    }
}
