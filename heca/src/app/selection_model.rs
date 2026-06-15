//! Host-level shared selection model.
//!
//! Selection is a host capability reusable across terminal panes, future
//! custom Neovim GUI panes, embedded browser panes, and host-native content
//! surfaces. This module defines the shared state and terminology; it does not
//! implement clipboard, paste, full keyboard selection mode, or
//! backend-native integration — those belong to later tasks.
//!
//! The model covers the shared concepts from [`terminal-implementation.md`]:
//! - selection owner
//! - selection source
//! - selection phase/state
//! - selection rendering mode

use heca_core::layout::PaneId;

/// The pane or surface that currently owns the active selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionOwner {
    /// A pane in the session. All current content surfaces are panes.
    Pane(PaneId),
}

/// How the current selection was initiated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionSource {
    /// Pointer drag gesture.
    MouseDrag,
    /// Keyboard-driven selection mode.
    KeyboardMode,
    /// RPC or other programmatic request.
    Rpc,
}

/// Lifecycle phase of a selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum SelectionPhase {
    /// No active selection.
    #[default]
    Inactive,
    /// Selection gesture in progress.
    Selecting,
    /// Selection gesture completed and confirmed.
    Selected,
}

/// Who renders the selection visual and owns the anchor semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionRenderMode {
    /// heca owns the text/grid model and renders the selection overlay.
    HostGrid,
    /// The backend engine owns selection semantics; the host only routes actions.
    BackendNative,
}

/// Mode-specific selection region.
///
/// The variant is chosen by the caller when a selection begins and cannot be
/// changed while the selection is active. This guarantees that anchor semantics
/// always match the declared render mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionRegion {
    /// Cell-based grid coordinate range for host-rendered selection.
    HostGrid {
        anchor_row: usize,
        anchor_col: usize,
        focus_row: usize,
        focus_col: usize,
    },
    /// Opaque backend-native region. The host does not interpret it.
    BackendNative,
}

impl SelectionRegion {
    /// Return the host-grid render mode this region belongs to.
    pub fn render_mode(&self) -> SelectionRenderMode {
        match self {
            SelectionRegion::HostGrid { .. } => SelectionRenderMode::HostGrid,
            SelectionRegion::BackendNative => SelectionRenderMode::BackendNative,
        }
    }

    /// Update only the focus end of a host-grid region.
    ///
    /// Has no effect for backend-native regions.
    pub fn with_focus(self, row: usize, col: usize) -> Self {
        match self {
            SelectionRegion::HostGrid { anchor_row, anchor_col, .. } => SelectionRegion::HostGrid {
                anchor_row,
                anchor_col,
                focus_row: row,
                focus_col: col,
            },
            SelectionRegion::BackendNative => SelectionRegion::BackendNative,
        }
    }
}

/// Active selection payload.
///
/// Only present while a selection is active. The render mode and region variant
/// are locked together by construction.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActiveSelection {
    pub owner: SelectionOwner,
    pub source: SelectionSource,
    pub region: SelectionRegion,
}

impl ActiveSelection {
    /// Who renders the selection visual.
    pub fn render_mode(&self) -> SelectionRenderMode {
        self.region.render_mode()
    }

    /// Update the focus end of the selection region in-place.
    ///
    /// For backend-native regions this is a no-op because the host does not
    /// interpret backend positions.
    pub fn update_focus(&mut self, row: usize, col: usize) {
        self.region = self.region.with_focus(row, col);
    }
}

/// Host-owned selection state.
///
/// Owned by [`crate::app_state::AppState`] and reused across pane/backend types.
/// The enum shape enforces that a selection payload exists exactly when the
/// phase is `Selecting` or `Selected`.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum SelectionState {
    /// No active selection.
    #[default]
    Inactive,
    /// Selection gesture in progress.
    Selecting(ActiveSelection),
    /// Selection gesture completed and confirmed.
    Selected(ActiveSelection),
}

impl SelectionState {
    /// Create an inactive selection state.
    pub fn new() -> Self {
        Self::default()
    }

    /// The surface that owns the active selection, if any.
    pub fn owner(&self) -> Option<SelectionOwner> {
        self.active().map(|a| a.owner)
    }

    /// Current lifecycle phase.
    pub fn phase(&self) -> SelectionPhase {
        match self {
            SelectionState::Inactive => SelectionPhase::Inactive,
            SelectionState::Selecting(_) => SelectionPhase::Selecting,
            SelectionState::Selected(_) => SelectionPhase::Selected,
        }
    }

    /// How the selection was initiated, if active.
    pub fn source(&self) -> Option<SelectionSource> {
        self.active().map(|a| a.source)
    }

    /// The active selection payload, if any.
    pub fn active(&self) -> Option<&ActiveSelection> {
        match self {
            SelectionState::Inactive => None,
            SelectionState::Selecting(active) | SelectionState::Selected(active) => Some(active),
        }
    }

    /// Who renders the selection visual, if a selection is active.
    pub fn render_mode(&self) -> Option<SelectionRenderMode> {
        self.active().map(|a| a.render_mode())
    }

    /// Selection region, if active.
    pub fn region(&self) -> Option<&SelectionRegion> {
        self.active().map(|a| &a.region)
    }

    /// True if a selection gesture or confirmed selection exists.
    pub fn is_active(&self) -> bool {
        !matches!(self, SelectionState::Inactive)
    }

    /// True if a selection gesture is currently in progress.
    pub fn is_selecting(&self) -> bool {
        matches!(self, SelectionState::Selecting(_))
    }

    /// True if a selection has been confirmed.
    pub fn has_selection(&self) -> bool {
        matches!(self, SelectionState::Selected(_))
    }

    /// Start a new selection gesture.
    ///
    /// Resets any previous selection state. The render mode is derived from the
    /// provided region, so the two cannot become inconsistent.
    pub fn begin(
        &mut self,
        owner: SelectionOwner,
        source: SelectionSource,
        region: SelectionRegion,
    ) {
        *self = SelectionState::Selecting(ActiveSelection {
            owner,
            source,
            region,
        });
    }

    /// Update the moving end of an in-progress host-grid selection.
    ///
    /// Has no effect if no selection is in progress or if the active selection
    /// is backend-native.
    pub fn update_focus(&mut self, row: usize, col: usize) {
        if let SelectionState::Selecting(active) = self {
            active.update_focus(row, col);
        }
    }

    /// Confirm the current in-progress selection.
    ///
    /// Has no effect if no selection is in progress.
    pub fn end(&mut self) {
        if let SelectionState::Selecting(active) = self {
            *self = SelectionState::Selected(active.clone());
        }
    }

    /// Clear the selection and return to inactive.
    pub fn clear(&mut self) {
        *self = SelectionState::Inactive;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_selection_is_inactive() {
        let s = SelectionState::new();
        assert!(!s.is_active());
        assert_eq!(s.phase(), SelectionPhase::Inactive);
        assert!(s.owner().is_none());
        assert!(s.active().is_none());
        assert!(s.render_mode().is_none());
        assert!(s.region().is_none());
    }

    #[test]
    fn default_matches_new() {
        assert_eq!(SelectionState::default(), SelectionState::new());
    }

    #[test]
    fn begin_transitions_to_selecting_host_grid() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 0,
            },
        );
        assert!(s.is_selecting());
        assert_eq!(s.owner(), Some(SelectionOwner::Pane(PaneId(1))));
        assert_eq!(s.source(), Some(SelectionSource::MouseDrag));
        assert_eq!(s.render_mode(), Some(SelectionRenderMode::HostGrid));
        assert_eq!(
            s.region(),
            Some(&SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 0,
            })
        );
    }

    #[test]
    fn begin_transitions_to_selecting_backend_native() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(2)),
            SelectionSource::Rpc,
            SelectionRegion::BackendNative,
        );
        assert!(s.is_selecting());
        assert_eq!(s.render_mode(), Some(SelectionRenderMode::BackendNative));
        assert_eq!(s.region(), Some(&SelectionRegion::BackendNative));
    }

    #[test]
    fn update_focus_only_while_selecting() {
        let mut s = SelectionState::new();
        s.update_focus(1, 2);
        assert!(s.region().is_none());
    }

    #[test]
    fn update_focus_no_op_when_selected() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 0,
            },
        );
        s.update_focus(0, 10);
        s.end();
        s.update_focus(0, 99);
        assert_eq!(
            s.region(),
            Some(&SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 10,
            })
        );
        assert!(s.has_selection());
    }

    #[test]
    fn update_focus_changes_host_grid_focus() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(7)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_row: 2,
                anchor_col: 3,
                focus_row: 2,
                focus_col: 3,
            },
        );
        s.update_focus(4, 5);
        assert_eq!(
            s.region(),
            Some(&SelectionRegion::HostGrid {
                anchor_row: 2,
                anchor_col: 3,
                focus_row: 4,
                focus_col: 5,
            })
        );
    }

    #[test]
    fn update_focus_is_no_op_for_backend_native() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::BackendNative,
        );
        s.update_focus(4, 5);
        assert_eq!(s.region(), Some(&SelectionRegion::BackendNative));
    }

    #[test]
    fn end_confirms_selection() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 0,
            },
        );
        s.update_focus(0, 5);
        s.end();
        assert!(s.has_selection());
        assert!(!s.is_selecting());
        assert_eq!(
            s.region(),
            Some(&SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 5,
            })
        );
    }

    #[test]
    fn end_has_no_effect_when_not_selecting() {
        let mut s = SelectionState::new();
        s.end();
        assert!(!s.has_selection());
        assert_eq!(s.phase(), SelectionPhase::Inactive);
    }

    #[test]
    fn end_is_idempotent_when_selected() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 5,
            },
        );
        s.end();
        assert!(s.has_selection());
        s.end();
        assert!(s.has_selection());
        assert_eq!(
            s.region(),
            Some(&SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 5,
            })
        );
    }

    #[test]
    fn clear_resets_state() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::Rpc,
            SelectionRegion::BackendNative,
        );
        s.end();
        s.clear();
        assert!(!s.is_active());
        assert_eq!(s.phase(), SelectionPhase::Inactive);
        assert!(s.owner().is_none());
        assert!(s.source().is_none());
        assert!(s.render_mode().is_none());
        assert!(s.region().is_none());
    }

    #[test]
    fn selection_region_render_mode_matches_variant() {
        assert_eq!(
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 1,
                focus_col: 2,
            }
            .render_mode(),
            SelectionRenderMode::HostGrid
        );
        assert_eq!(
            SelectionRegion::BackendNative.render_mode(),
            SelectionRenderMode::BackendNative
        );
    }

    #[test]
    fn active_selection_render_mode_derives_from_region() {
        let active = ActiveSelection {
            owner: SelectionOwner::Pane(PaneId(1)),
            source: SelectionSource::Rpc,
            region: SelectionRegion::BackendNative,
        };
        assert_eq!(active.render_mode(), SelectionRenderMode::BackendNative);
    }

    #[test]
    fn begin_resets_prior_selection() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 0,
            },
        );
        s.update_focus(0, 10);
        s.end();
        s.begin(
            SelectionOwner::Pane(PaneId(2)),
            SelectionSource::Rpc,
            SelectionRegion::BackendNative,
        );
        assert!(s.is_selecting());
        assert_eq!(s.owner(), Some(SelectionOwner::Pane(PaneId(2))));
        assert_eq!(s.source(), Some(SelectionSource::Rpc));
        assert_eq!(s.render_mode(), Some(SelectionRenderMode::BackendNative));
        assert_eq!(s.region(), Some(&SelectionRegion::BackendNative));
    }
}
