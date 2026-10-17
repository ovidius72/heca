//! The terminal text selection's **state model** — who owns it, how it was
//! started, and the little state machine (inactive → caret → selecting →
//! selected). Pure state: it owns no terminal grid, no clipboard, and no host
//! wiring.

use crate::chrome::panes::terminal::TerminalId;

/// **The terminal that owns the active selection** — any terminal, whoever owns it: a pane's, or
/// one an extension placed in a dock or an overlay. A pane is asked for its terminal's id, so there
/// is one owner and one path, not a pane kind and a second kind for the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SelectionOwner(pub TerminalId);

/// How the current selection was initiated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionSource {
    /// Pointer drag gesture.
    MouseDrag,
    /// Keyboard-driven selection mode.
    KeyboardMode,
}

/// Where a selection is: inclusive anchor/focus cell coordinates in stable-row
/// space. The anchor is fixed when the selection begins; the focus is the end
/// that movement grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SelectionRegion {
    pub anchor_stable_row: isize,
    pub anchor_col: usize,
    pub focus_stable_row: isize,
    pub focus_col: usize,
}

impl SelectionRegion {
    /// Update only the focus end, leaving the anchor fixed.
    pub fn with_focus(self, stable_row: isize, col: usize) -> Self {
        SelectionRegion {
            focus_stable_row: stable_row,
            focus_col: col,
            ..self
        }
    }
}

/// Active selection payload.
///
/// Only present while a selection is active.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActiveSelection {
    pub owner: SelectionOwner,
    pub source: SelectionSource,
    pub region: SelectionRegion,
}

impl ActiveSelection {
    /// Update the focus end of the selection region in-place.
    pub fn update_focus(&mut self, stable_row: isize, col: usize) {
        self.region = self.region.with_focus(stable_row, col);
    }
}

/// Host-owned selection state.
///
/// Owned by [`crate::app_state::AppState`] and reused across pane/backend types.
/// The enum shape enforces that a selection payload exists exactly when the
/// state is `Selecting` or `Selected`. The `Caret` variant represents a
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
        stable_row: isize,
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

    /// Selection region, if active.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "used by tests and reserved for upcoming selection wiring"
        )
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
        expect(
            dead_code,
            reason = "used by tests and reserved for upcoming confirmed-selection flows"
        )
    )]
    pub fn has_selection(&self) -> bool {
        matches!(self, SelectionState::Selected(_))
    }

    /// True if in caret-only state (selection mode entered but no selection started).
    pub fn is_caret(&self) -> bool {
        matches!(self, SelectionState::Caret { .. })
    }

    /// Return the caret position, if in caret-only state.
    pub fn caret_pos(&self) -> Option<(isize, usize)> {
        match self {
            SelectionState::Caret {
                stable_row, col, ..
            } => Some((*stable_row, *col)),
            _ => None,
        }
    }

    /// The current cursor cell — the caret in caret-only state, or the moving
    /// focus endpoint of an active selection — with its owner, in any active
    /// state. `None` when inactive. Used by follow-link-at-caret (`O` in
    /// selection mode).
    pub fn cursor_cell(&self) -> Option<(SelectionOwner, isize, usize)> {
        match self {
            SelectionState::Inactive => None,
            SelectionState::Caret {
                owner,
                stable_row,
                col,
            } => Some((*owner, *stable_row, *col)),
            SelectionState::Selecting(active) | SelectionState::Selected(active) => Some((
                active.owner,
                active.region.focus_stable_row,
                active.region.focus_col,
            )),
        }
    }

    /// Place a caret at the given position (caret-only state, no selection).
    pub fn set_caret(&mut self, owner: SelectionOwner, stable_row: isize, col: usize) {
        *self = SelectionState::Caret {
            owner,
            stable_row,
            col,
        };
    }

    /// Move the caret position. No-op if not in caret-only state.
    pub fn move_caret(&mut self, stable_row: isize, col: usize) {
        if let SelectionState::Caret {
            stable_row: r,
            col: c,
            ..
        } = self
        {
            *r = stable_row;
            *c = col;
        }
    }

    /// Begin selection from the caret position. No-op if not in caret state.
    ///
    /// The caret becomes the anchor AND focus of the new selection.
    /// After this, movement will grow the selection from the anchor.
    pub fn begin_selection_from_caret(&mut self, source: SelectionSource) {
        if let SelectionState::Caret {
            owner,
            stable_row,
            col,
        } = *self
        {
            *self = SelectionState::Selecting(ActiveSelection {
                owner,
                source,
                region: SelectionRegion {
                    anchor_stable_row: stable_row,
                    anchor_col: col,
                    focus_stable_row: stable_row,
                    focus_col: col,
                },
            });
        }
    }

    /// Toggle which endpoint of the selection is active (anchor vs focus).
    ///
    /// After toggling, movement keys will grow the selection from the other end.
    /// No-op if not in Selecting or Selected state.
    pub fn toggle_selection_endpoint(&mut self) {
        if let SelectionState::Selecting(active) | SelectionState::Selected(active) = self {
            let region = &mut active.region;
            std::mem::swap(&mut region.anchor_stable_row, &mut region.focus_stable_row);
            std::mem::swap(&mut region.anchor_col, &mut region.focus_col);
        }
    }

    /// Start a new selection gesture.
    ///
    /// Resets any previous selection state.
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

    /// Update the moving end of an in-progress selection.
    ///
    /// Has no effect if no selection is in progress.
    pub fn update_focus(&mut self, stable_row: isize, col: usize) {
        if let SelectionState::Selecting(active) = self {
            active.update_focus(stable_row, col);
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
