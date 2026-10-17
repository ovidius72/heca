//! The terminal's text selection: the state model, copying it out, and the host
//! operations that drive it from a terminal's grid.

mod drive;
mod extract;
mod model;

#[cfg(test)]
mod extract_tests;
#[cfg(test)]
mod model_tests;

pub use model::SelectionState;
pub(crate) use extract::extract_selection_text;
pub(crate) use model::{SelectionOwner, SelectionRegion, SelectionSource};
pub(crate) use drive::{
    ensure_caret_visible, enter_selection_mode_for_focused_terminal, move_focused_terminal_selection,
};
pub(in crate::app::terminal_host) use drive::{begin_terminal_selection_at, visible_row_to_stable_row};
