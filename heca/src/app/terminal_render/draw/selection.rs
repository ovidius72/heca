//! The selection and caret a terminal is drawn with.

use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionState};
use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;
use heca_config::theme::Color;
use heca_core::backend::TerminalSnapshot;
use heca_renderer::terminal::{CaretIndicator, SelectionOverlay, SelectionOverlaySpan};

pub(super) fn selection_overlay_for_terminal(
    state: &AppState,
    terminal: TerminalId,
    snapshot: &TerminalSnapshot,
) -> Option<SelectionOverlay> {
    build_selection_overlay(&state.selection, terminal, snapshot, &state.theme.accent)
}

// `pub(super)` so the selection-overlay unit tests (in `app::render`) can reach it
// after #118 moved this fn out of `render.rs`.
pub(super) fn build_selection_overlay(
    selection: &SelectionState,
    terminal: TerminalId,
    snapshot: &TerminalSnapshot,
    accent: &Color,
) -> Option<SelectionOverlay> {
    if snapshot.cols == 0 || snapshot.rows == 0 {
        return None;
    }

    let stable_to_visible = |stable_row: isize| -> Option<usize> {
        let visible = stable_row - snapshot.viewport_top_stable_row;
        usize::try_from(visible)
            .ok()
            .filter(|row| *row < snapshot.rows)
    };

    if let SelectionState::Caret {
        owner,
        stable_row,
        col,
        ..
    } = selection
    {
        if *owner != SelectionOwner(terminal) {
            return None;
        }
        let row = stable_to_visible(*stable_row)?;
        let color = [
            accent.r as f32 / 255.0,
            accent.g as f32 / 255.0,
            accent.b as f32 / 255.0,
            0.25,
        ];
        return Some(
            SelectionOverlay::new(vec![], color).with_caret(CaretIndicator {
                row,
                col: *col,
                is_selection_endpoint: false,
            }),
        );
    }

    let active = selection.active()?;
    if active.owner != SelectionOwner(terminal) {
        return None;
    }
    match &active.region {
        SelectionRegion::HostGrid {
            anchor_stable_row,
            anchor_col,
            focus_stable_row,
            focus_col,
        } => {
            let color = [
                accent.r as f32 / 255.0,
                accent.g as f32 / 255.0,
                accent.b as f32 / 255.0,
                0.25,
            ];
            let last_col = snapshot.cols.saturating_sub(1);
            let start_stable = (*anchor_stable_row).min(*focus_stable_row);
            let end_stable = (*anchor_stable_row).max(*focus_stable_row);
            let start_col = if anchor_stable_row < focus_stable_row {
                *anchor_col
            } else if focus_stable_row < anchor_stable_row {
                *focus_col
            } else {
                (*anchor_col).min(*focus_col)
            };
            let end_col = if focus_stable_row > anchor_stable_row {
                *focus_col
            } else if anchor_stable_row > focus_stable_row {
                *anchor_col
            } else {
                (*anchor_col).max(*focus_col)
            };

            let visible_start = snapshot.viewport_top_stable_row.max(start_stable);
            let visible_end =
                (snapshot.viewport_top_stable_row + snapshot.rows as isize - 1).min(end_stable);
            let mut spans = Vec::new();
            if visible_start <= visible_end {
                for stable_row in visible_start..=visible_end {
                    let row = stable_to_visible(stable_row)
                        .expect("visible stable row must convert to a visible row");
                    let (s, e) = if start_stable == end_stable {
                        (start_col.min(last_col), end_col.min(last_col))
                    } else if stable_row == start_stable {
                        (start_col.min(last_col), last_col)
                    } else if stable_row == end_stable {
                        (0, end_col.min(last_col))
                    } else {
                        (0, last_col)
                    };
                    if s <= e {
                        spans.push(SelectionOverlaySpan {
                            row,
                            start_col: s,
                            end_col: e,
                        });
                    }
                }
            }

            let caret = stable_to_visible(*focus_stable_row).map(|row| CaretIndicator {
                row,
                col: *focus_col,
                is_selection_endpoint: true,
            });
            let overlay = SelectionOverlay::new(spans, color);
            Some(match caret {
                Some(caret) => overlay.with_caret(caret),
                None => overlay,
            })
        }
        SelectionRegion::BackendNative => None,
    }
}

#[cfg(test)]
mod tests;
