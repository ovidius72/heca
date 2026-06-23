//! Host-level shared selection model.
//!
//! Selection is a host capability reusable across terminal panes, future
//! custom Neovim GUI panes, embedded browser panes, and host-native content
//! surfaces. This module defines the shared state and terminology; it does not
//! implement paste, bracketed-paste, or `OSC 52` — those belong to later tasks.
//!
//! The model covers the shared concepts from [`terminal-implementation.md`]:
//! - selection owner
//! - selection source
//! - selection phase/state
//! - selection rendering mode
//!
//! Text extraction for copy is also here: [`extract_selection_text`] is a
//! pure function that converts host-grid selection coordinates into a `String`
//! using terminal snapshot lines. It knows the selection contract (inclusive
//! cell coordinates, wide-char filler handling, row-trailing whitespace trimming)
//! but does not own clipboard I/O.

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
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "RPC-originated selection is reserved for upcoming wiring")
    )]
    Rpc,
}

/// Lifecycle phase of a selection.
///
/// Reserved for the planned RPC/backend-native selection flow. Today the app
/// matches directly on [`SelectionState`], but the phase enum stays in sync with
/// the shared selection contract that the next task will wire up.
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
///
/// Reserved for the planned RPC/backend-native selection flow.
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
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "backend-native region is reserved for future backend-owned selection")
    )]
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
            SelectionRegion::HostGrid {
                anchor_row,
                anchor_col,
                ..
            } => SelectionRegion::HostGrid {
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
/// phase is `Selecting` or `Selected`. The `Caret` variant represents a
/// caret-only state (selection mode entered but no selection started yet).
#[derive(Clone, Debug, PartialEq, Default)]
pub enum SelectionState {
    /// No active selection and no caret.
    #[default]
    Inactive,
    /// Caret-only: selection mode is active but no selection has been started.
    /// Movement updates the caret position; `v`/`Space` begins selection from here.
    Caret {
        owner: SelectionOwner,
        row: usize,
        col: usize,
    },
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
        match self {
            SelectionState::Inactive => None,
            SelectionState::Caret { owner, .. } => Some(*owner),
            SelectionState::Selecting(active) | SelectionState::Selected(active) => {
                Some(active.owner)
            }
        }
    }

    /// Current lifecycle phase.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by tests and reserved for future selection lifecycle routing")
    )]
    pub fn phase(&self) -> SelectionPhase {
        match self {
            SelectionState::Inactive => SelectionPhase::Inactive,
            SelectionState::Caret { .. } => SelectionPhase::Inactive,
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
            SelectionState::Inactive | SelectionState::Caret { .. } => None,
            SelectionState::Selecting(active) | SelectionState::Selected(active) => Some(active),
        }
    }

    /// Who renders the selection visual, if a selection is active.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by tests and reserved for backend-native selection wiring")
    )]
    pub fn render_mode(&self) -> Option<SelectionRenderMode> {
        self.active().map(|a| a.render_mode())
    }

    /// Selection region, if active.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by tests and reserved for backend-native selection wiring")
    )]
    pub fn region(&self) -> Option<&SelectionRegion> {
        self.active().map(|a| &a.region)
    }

    /// True if a selection gesture or confirmed selection exists (excludes caret-only).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            SelectionState::Selecting(_) | SelectionState::Selected(_)
        )
    }

    /// True if a selection gesture is currently in progress.
    pub fn is_selecting(&self) -> bool {
        matches!(self, SelectionState::Selecting(_))
    }

    /// True if a selection has been confirmed.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by tests and reserved for upcoming confirmed-selection flows")
    )]
    pub fn has_selection(&self) -> bool {
        matches!(self, SelectionState::Selected(_))
    }

    /// True if in caret-only state (selection mode entered but no selection started).
    pub fn is_caret(&self) -> bool {
        matches!(self, SelectionState::Caret { .. })
    }

    /// Return the caret position, if in caret-only state.
    pub fn caret_pos(&self) -> Option<(usize, usize)> {
        match self {
            SelectionState::Caret { row, col, .. } => Some((*row, *col)),
            _ => None,
        }
    }

    /// Place a caret at the given position (caret-only state, no selection).
    pub fn set_caret(&mut self, owner: SelectionOwner, row: usize, col: usize) {
        *self = SelectionState::Caret { owner, row, col };
    }

    /// Move the caret position. No-op if not in caret-only state.
    pub fn move_caret(&mut self, row: usize, col: usize) {
        if let SelectionState::Caret { row: r, col: c, .. } = self {
            *r = row;
            *c = col;
        }
    }

    /// Begin selection from the caret position. No-op if not in caret state.
    ///
    /// The caret becomes the anchor AND focus of the new selection.
    /// After this, movement will grow the selection from the anchor.
    pub fn begin_selection_from_caret(&mut self, source: SelectionSource) {
        if let SelectionState::Caret { owner, row, col } = *self {
            *self = SelectionState::Selecting(ActiveSelection {
                owner,
                source,
                region: SelectionRegion::HostGrid {
                    anchor_row: row,
                    anchor_col: col,
                    focus_row: row,
                    focus_col: col,
                },
            });
        }
    }

    /// Toggle which endpoint of the selection is active (anchor vs focus).
    ///
    /// After toggling, movement keys will grow the selection from the other end.
    /// No-op if not in Selecting or Selected state with a HostGrid region.
    pub fn toggle_selection_endpoint(&mut self) {
        match self {
            SelectionState::Selecting(ActiveSelection {
                region:
                    SelectionRegion::HostGrid {
                        anchor_row,
                        anchor_col,
                        focus_row,
                        focus_col,
                    },
                ..
            })
            | SelectionState::Selected(ActiveSelection {
                region:
                    SelectionRegion::HostGrid {
                        anchor_row,
                        anchor_col,
                        focus_row,
                        focus_col,
                    },
                ..
            }) => {
                std::mem::swap(anchor_row, focus_row);
                std::mem::swap(anchor_col, focus_col);
            }
            _ => {}
        }
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
    fn caret_state_is_pre_selection() {
        let mut s = SelectionState::new();
        s.set_caret(SelectionOwner::Pane(PaneId(1)), 3, 5);
        // Caret is not "active" (no selection gesture), but it has an owner.
        assert!(!s.is_active());
        assert!(s.is_caret());
        assert_eq!(s.phase(), SelectionPhase::Inactive);
        assert_eq!(s.owner(), Some(SelectionOwner::Pane(PaneId(1))));
        assert!(s.active().is_none()); // no selection payload
        assert_eq!(s.caret_pos(), Some((3, 5)));
    }

    #[test]
    fn caret_move_updates_position() {
        let mut s = SelectionState::new();
        s.set_caret(SelectionOwner::Pane(PaneId(1)), 0, 0);
        s.move_caret(2, 7);
        assert_eq!(s.caret_pos(), Some((2, 7)));
    }

    #[test]
    fn caret_move_is_no_op_when_not_caret() {
        let mut s = SelectionState::new();
        // Inactive state — move_caret does nothing.
        s.move_caret(5, 5);
        assert_eq!(s.caret_pos(), None);
    }

    #[test]
    fn begin_selection_from_caret() {
        let mut s = SelectionState::new();
        s.set_caret(SelectionOwner::Pane(PaneId(1)), 3, 5);
        s.begin_selection_from_caret(SelectionSource::KeyboardMode);
        // Now in Selecting state with anchor and focus at the caret position.
        assert!(s.is_active());
        assert!(!s.is_caret());
        assert!(s.is_selecting());
        let active = s.active().unwrap();
        assert_eq!(active.owner, SelectionOwner::Pane(PaneId(1)));
        assert_eq!(active.source, SelectionSource::KeyboardMode);
        match &active.region {
            SelectionRegion::HostGrid {
                anchor_row,
                anchor_col,
                focus_row,
                focus_col,
            } => {
                assert_eq!(
                    (*anchor_row, *anchor_col, *focus_row, *focus_col),
                    (3, 5, 3, 5)
                );
            }
            _ => panic!("expected HostGrid region"),
        }
    }

    #[test]
    fn begin_selection_from_caret_no_op_when_not_caret() {
        let mut s = SelectionState::new();
        s.begin_selection_from_caret(SelectionSource::KeyboardMode);
        // Still inactive — no-op.
        assert!(!s.is_active());
    }

    #[test]
    fn toggle_selection_endpoint_swaps_anchor_and_focus() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 2,
                focus_row: 3,
                focus_col: 5,
            },
        );
        s.toggle_selection_endpoint();
        match s.active().unwrap().region {
            SelectionRegion::HostGrid {
                anchor_row,
                anchor_col,
                focus_row,
                focus_col,
            } => {
                // After toggle, anchor and focus are swapped.
                assert_eq!((anchor_row, anchor_col, focus_row, focus_col), (3, 5, 0, 2));
            }
            _ => panic!("expected HostGrid region"),
        }
    }

    #[test]
    fn toggle_endpoint_no_op_on_backend_native() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::Rpc,
            SelectionRegion::BackendNative,
        );
        s.toggle_selection_endpoint();
        // Still BackendNative — no crash, no change.
        assert!(matches!(s.region(), Some(SelectionRegion::BackendNative)));
    }

    #[test]
    fn toggle_endpoint_no_op_on_caret() {
        let mut s = SelectionState::new();
        s.set_caret(SelectionOwner::Pane(PaneId(1)), 0, 0);
        s.toggle_selection_endpoint();
        // Still in caret state.
        assert!(s.is_caret());
    }

    #[test]
    fn toggle_endpoint_works_on_selected_state() {
        let mut s = SelectionState::new();
        s.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_row: 1,
                anchor_col: 0,
                focus_row: 5,
                focus_col: 3,
            },
        );
        s.end(); // Confirm → Selected state
        assert!(s.has_selection());
        s.toggle_selection_endpoint();
        match s.active().unwrap().region {
            SelectionRegion::HostGrid {
                anchor_row,
                anchor_col,
                focus_row,
                focus_col,
            } => {
                assert_eq!((anchor_row, anchor_col, focus_row, focus_col), (5, 3, 1, 0));
            }
            _ => panic!("expected HostGrid region"),
        }
    }

    #[test]
    fn clear_resets_caret_to_inactive() {
        let mut s = SelectionState::new();
        s.set_caret(SelectionOwner::Pane(PaneId(1)), 0, 0);
        s.clear();
        assert!(!s.is_caret());
        assert!(!s.is_active());
        assert_eq!(s.phase(), SelectionPhase::Inactive);
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

// ── Text extraction ──

/// Extract selected text from terminal snapshot lines using the active
/// host-grid selection.
///
/// # Contract
///
/// - Host-grid selection coordinates are **inclusive** cell coordinates.
/// - Single-row selection copies cells from `start_col..=end_col`.
/// - Multi-row selection copies:
///   - first row: `start_col..end_of_row` (exclusive of trailing blank filler)
///   - middle rows: full logical selected row span
///   - last row: `0..=end_col`
/// - Rows are joined with `\n`. No trailing newline after the last row.
/// - Wide-character filler cells (width == 0 following a width > 1 anchor) are
///   skipped — only the logical anchor cell text is copied.
/// - Internal spaces are preserved exactly.
/// - Trailing whitespace is trimmed per-row only for cells that are visually
///   blank (text is empty or all-ASCII whitespace and the cell has the default
///   background color). This avoids copying the visual padding that terminals
///   add to fill the grid width.
/// - Returns `None` when the selection is inactive or uses `BackendNative`
///   rendering.
/// - The owner is assumed to be `SelectionOwner::Pane` (the only variant today);
///   if future owner variants are added, this function must be updated to handle
///   or reject them.
pub fn extract_selection_text(
    selection: &SelectionState,
    lines: &[heca_core::backend::TerminalLine],
    cols: usize,
    default_bg: [f32; 4],
) -> Option<String> {
    let active = selection.active()?;
    match &active.region {
        SelectionRegion::HostGrid {
            anchor_row,
            anchor_col,
            focus_row,
            focus_col,
        } => {
            let (start_row, start_col, end_row, end_col) =
                normalize_selection_rect(*anchor_row, *anchor_col, *focus_row, *focus_col);
            Some(extract_text_from_grid(
                lines, cols, start_row, start_col, end_row, end_col, default_bg,
            ))
        }
        SelectionRegion::BackendNative => {
            // Backend-native selection text extraction is not supported yet.
            // The host does not own the text model for backend-native selections.
            None
        }
    }
}

/// Normalize anchor/focus into top-left / bottom-right inclusive bounds.
fn normalize_selection_rect(
    anchor_row: usize,
    anchor_col: usize,
    focus_row: usize,
    focus_col: usize,
) -> (usize, usize, usize, usize) {
    let start_row = anchor_row.min(focus_row);
    let start_col = if anchor_row < focus_row {
        anchor_col
    } else if focus_row < anchor_row {
        focus_col
    } else {
        anchor_col.min(focus_col)
    };
    let end_row = anchor_row.max(focus_row);
    let end_col = if focus_row > anchor_row {
        focus_col
    } else if anchor_row > focus_row {
        anchor_col
    } else {
        anchor_col.max(focus_col)
    };
    (start_row, start_col, end_row, end_col)
}

/// Extract text from a terminal grid region defined by inclusive cell coordinates.
///
/// Follows the selection extraction contract documented on [`extract_selection_text`].
fn extract_text_from_grid(
    lines: &[heca_core::backend::TerminalLine],
    cols: usize,
    start_row: usize,
    start_col: usize,
    end_row: usize,
    end_col: usize,
    default_bg: [f32; 4],
) -> String {
    if lines.is_empty() || cols == 0 {
        return String::new();
    }
    let mut result = String::new();
    for row in start_row..=end_row {
        let line = match lines.get(row) {
            Some(l) => l,
            None => {
                // Row beyond grid — skip it rather than panic.
                continue;
            }
        };
        let (row_start, row_end) = if start_row == end_row {
            // Single-row selection: inclusive both ends.
            (start_col, end_col)
        } else if row == start_row {
            // First row of multi-row: from start_col to end of row.
            (start_col, cols.saturating_sub(1))
        } else if row == end_row {
            // Last row of multi-row: from beginning to end_col inclusive.
            (0, end_col)
        } else {
            // Middle row: full row.
            (0, cols.saturating_sub(1))
        };
        let row_text = extract_row_text(line, row_start, row_end, default_bg);
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&row_text);
    }
    result
}

/// Extract text from a single terminal line within the given column range
/// (inclusive on both ends).
///
/// Skips wide-character filler cells (width == 0) — only the anchor cell
/// (width >= 1) text is copied. Trailing blank cells (empty text, default
/// background) are trimmed from the right.
fn extract_row_text(
    line: &heca_core::backend::TerminalLine,
    start_col: usize,
    end_col: usize,
    default_bg: [f32; 4],
) -> String {
    let cells = &line.cells;
    let mut result = String::new();
    let mut last_content_end = 0usize; // tracks rightmost non-blank cell index

    // Adjust start_col backward if it lands on a wide-char filler cell.
    // If start_col points at a filler (width == 0), the anchor wide character
    // is to its left. Most terminal emulators widen the selection to include
    // the full wide character. Walk back to find and include it.
    let adjusted_start = if start_col > 0 {
        let mut adj = start_col;
        while adj > 0 && cells.get(adj).is_some_and(|c| c.width == 0) {
            adj -= 1;
        }
        adj
    } else {
        start_col
    };

    let mut i = 0usize;
    while i < cells.len() {
        if i > end_col {
            break;
        }
        let cell = &cells[i];
        if cell.width == 0 {
            // Wide-char filler cell — skip it.
            i += 1;
            continue;
        }
        if i >= adjusted_start && i <= end_col {
            // This is a content cell within the selection range.
            if !is_trailing_blank(cell, default_bg) {
                // Extend the last-content marker; we'll trim truly trailing blanks
                // after the loop.
                last_content_end = result.len() + cell.text.len();
            }
            result.push_str(&cell.text);
        }
        i += 1;
    }
    // Trim trailing whitespace that corresponds to visually blank filler cells
    // at the end of the row. We only trim the part after `last_content_end`.
    result.truncate(last_content_end);
    result
}

/// A cell is considered "trailing blank" if it has empty/whitespace-only text
/// AND its background matches the terminal's default background. These are the
/// visual padding cells terminals use to fill the grid width.
///
/// Only cells that are both text-empty and default-bg are trimmed — intentional
/// selected whitespace (e.g. spaces in code) with a different background color
/// is preserved exactly, even at the end of a row.
fn is_trailing_blank(cell: &heca_core::backend::TerminalCell, default_bg: [f32; 4]) -> bool {
    cell.text.trim().is_empty() && is_default_bg(cell.bg, default_bg)
}

/// Check if a cell's background matches the terminal's default background.
///
/// Compares with a small epsilon tolerance for floating-point color values.
/// This distinguishes visually-blank filler cells (which use default_bg) from
/// cells that intentionally have whitespace content but a different background
/// (e.g. highlighted spaces in editors, which should be preserved).
fn is_default_bg(bg: [f32; 4], default_bg: [f32; 4]) -> bool {
    const EPS: f32 = 0.01;
    (bg[0] - default_bg[0]).abs() < EPS
        && (bg[1] - default_bg[1]).abs() < EPS
        && (bg[2] - default_bg[2]).abs() < EPS
        && (bg[3] - default_bg[3]).abs() < EPS
}

#[cfg(test)]
mod extraction_tests {
    use super::*;
    use heca_core::backend::{TerminalCell, TerminalLine};

    /// Default background color used in extraction tests: opaque dark (#030730).
    /// Cells with this background are considered visually-blank filler.
    const TEST_BG: [f32; 4] = [0.012, 0.027, 0.188, 1.0];

    /// A different background color — cells with this are NOT default-bg filler
    /// even if they have empty text, so they should be preserved in selection.
    const HIGHLIGHT_BG: [f32; 4] = [0.2, 0.3, 0.5, 1.0];

    fn cell(text: &str, width: usize) -> TerminalCell {
        TerminalCell {
            text: text.to_string(),
            fg: [1.0, 1.0, 1.0, 1.0],
            bg: TEST_BG,
            bold: false,
            italic: false,
            underline: heca_core::backend::TerminalUnderlineStyle::None,
            width,
        }
    }

    /// Cell with empty text and default background — a visual filler cell.
    fn blank_cell() -> TerminalCell {
        TerminalCell {
            text: String::new(),
            fg: [1.0, 1.0, 1.0, 1.0],
            bg: TEST_BG,
            bold: false,
            italic: false,
            underline: heca_core::backend::TerminalUnderlineStyle::None,
            width: 1,
        }
    }

    /// Cell with a space character but highlighted (non-default) background.
    /// This simulates a space that was intentionally selected — it must NOT
    /// be trimmed as trailing blank filler.
    fn highlighted_space_cell() -> TerminalCell {
        TerminalCell {
            text: " ".to_string(),
            fg: [1.0, 1.0, 1.0, 1.0],
            bg: HIGHLIGHT_BG,
            bold: false,
            italic: false,
            underline: heca_core::backend::TerminalUnderlineStyle::None,
            width: 1,
        }
    }

    /// Wide-character filler cell (width == 0). Appears after a wide grapheme
    /// anchor. Must be skipped during text extraction.
    fn filler_cell() -> TerminalCell {
        TerminalCell {
            text: String::new(),
            fg: [1.0, 1.0, 1.0, 1.0],
            bg: TEST_BG,
            bold: false,
            italic: false,
            underline: heca_core::backend::TerminalUnderlineStyle::None,
            width: 0,
        }
    }

    fn line(cells: Vec<TerminalCell>) -> TerminalLine {
        TerminalLine { cells }
    }

    #[test]
    fn extract_single_line_selection() {
        let lines = vec![line(vec![
            cell("h", 1),
            cell("e", 1),
            cell("l", 1),
            cell("l", 1),
            cell("o", 1),
            blank_cell(),
            blank_cell(),
        ])];
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 4,
            },
        );
        sel.end();
        let text = extract_selection_text(&sel, &lines, 7, TEST_BG).unwrap();
        assert_eq!(text, "hello");
    }

    #[test]
    fn extract_multi_line_selection() {
        let lines = vec![
            line(vec![cell("a", 1), cell("b", 1), cell("c", 1), blank_cell()]),
            line(vec![cell("d", 1), cell("e", 1), blank_cell(), blank_cell()]),
            line(vec![cell("f", 1), cell("g", 1), cell("h", 1), blank_cell()]),
        ];
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 1,
                focus_row: 2,
                focus_col: 1,
            },
        );
        sel.end();
        // Row 0: start_col=1, end=3 → "bc" (trailing blank trimmed)
        // Row 1: full → "de"
        // Row 2: 0..=1 → "fg"
        let text = extract_selection_text(&sel, &lines, 4, TEST_BG).unwrap();
        assert_eq!(text, "bc\nde\nfg");
    }

    #[test]
    fn extract_reverse_selection() {
        let lines = vec![
            line(vec![cell("x", 1), cell("y", 1), cell("z", 1)]),
            line(vec![cell("1", 1), cell("2", 1), cell("3", 1)]),
        ];
        // Anchor at bottom-right, focus at top-left → same text as forward.
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_row: 1,
                anchor_col: 2,
                focus_row: 0,
                focus_col: 0,
            },
        );
        sel.end();
        let text = extract_selection_text(&sel, &lines, 3, TEST_BG).unwrap();
        // Normalized: start=(0,0) end=(1,2)
        // Row 0: full → "xyz"
        // Row 1: 0..=2 → "123"
        assert_eq!(text, "xyz\n123");
    }

    #[test]
    fn extract_wide_char_skips_filler() {
        // Wide character '中' takes 2 cells; the second cell has width=0 (filler).
        let lines = vec![line(vec![
            cell("中", 2),
            blank_cell(),
            cell("b", 1),
            blank_cell(),
        ])];
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 2,
            },
        );
        sel.end();
        // Should copy "中b" (filler cell at col 1 is skipped)
        let text = extract_selection_text(&sel, &lines, 4, TEST_BG).unwrap();
        assert_eq!(text, "中b");
    }

    #[test]
    fn extract_inactive_selection_returns_none() {
        let sel = SelectionState::new();
        let lines: Vec<TerminalLine> = vec![];
        assert!(extract_selection_text(&sel, &lines, 0, TEST_BG).is_none());
    }

    #[test]
    fn extract_backend_native_returns_none() {
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::Rpc,
            SelectionRegion::BackendNative,
        );
        let lines: Vec<TerminalLine> = vec![];
        assert!(extract_selection_text(&sel, &lines, 0, TEST_BG).is_none());
    }

    #[test]
    fn extract_text_with_trailing_blanks_trimmed() {
        let lines = vec![line(vec![
            cell("a", 1),
            cell("b", 1),
            blank_cell(),
            blank_cell(),
        ])];
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 3,
            },
        );
        sel.end();
        // Selecting entire row including trailing blanks — should get "ab"
        let text = extract_selection_text(&sel, &lines, 4, TEST_BG).unwrap();
        assert_eq!(text, "ab");
    }

    #[test]
    fn extract_trailing_spaces_with_non_default_bg_are_preserved() {
        // Trailing spaces with a highlighted (non-default) background must NOT
        // be trimmed — they were intentionally selected.
        let lines = vec![line(vec![
            cell("a", 1),
            cell(" ", 1),
            highlighted_space_cell(),
        ])];
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 2,
            },
        );
        sel.end();
        // "a" + space + highlighted_space — both spaces must be preserved.
        let text = extract_selection_text(&sel, &lines, 3, TEST_BG).unwrap();
        assert_eq!(text, "a  ");
    }

    #[test]
    fn extract_default_bg_trailing_blanks_are_trimmed() {
        // Trailing cells with empty text AND default_bg are filler and trimmed.
        let lines = vec![line(vec![
            cell("a", 1),
            cell("b", 1),
            blank_cell(),
            blank_cell(),
        ])];
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 0,
                focus_row: 0,
                focus_col: 3,
            },
        );
        sel.end();
        // Trailing blank_cell() has default_bg, so it's trimmed.
        let text = extract_selection_text(&sel, &lines, 4, TEST_BG).unwrap();
        assert_eq!(text, "ab");
    }

    #[test]
    fn extract_wide_char_partial_selection_includes_anchor() {
        // If selection starts at a filler cell (width == 0), walk back to
        // include the anchor wide character.
        let lines = vec![line(vec![
            cell("\u{4e2d}", 2),
            filler_cell(),
            cell("b", 1),
            blank_cell(),
        ])];
        let mut sel = SelectionState::new();
        sel.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            // Start at column 1 (the filler of '中') — should include '中'
            SelectionRegion::HostGrid {
                anchor_row: 0,
                anchor_col: 1,
                focus_row: 0,
                focus_col: 2,
            },
        );
        sel.end();
        // Normalized: start=(0,1), end=(0,2)
        // The filler at col 1 walks back to col 0 (the anchor '中')
        // Expected: "\u{4e2d}b" (wide char included, filler skipped)
        let text = extract_selection_text(&sel, &lines, 4, TEST_BG).unwrap();
        assert_eq!(text, "\u{4e2d}b");
    }
}
