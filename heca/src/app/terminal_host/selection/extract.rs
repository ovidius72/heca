//! **Copying a selection out as text** — turning an active selection plus a
//! terminal's grid rows into a `String`. It knows the selection contract
//! (inclusive cell coordinates, wide-char filler, trailing-blank trimming);
//! it does not own clipboard I/O.

use super::{SelectionRegion, SelectionState};

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
/// - Returns `None` when the selection is inactive.
/// - The owner is whichever terminal the selection is in; `lines` must be that terminal's.
pub fn extract_selection_text(
    selection: &SelectionState,
    lines: &[heca_core::backend::TerminalLine],
    base_stable: isize,
    cols: usize,
    default_bg: [f32; 4],
) -> Option<String> {
    let active = selection.active()?;
    let SelectionRegion {
        anchor_stable_row,
        anchor_col,
        focus_stable_row,
        focus_col,
    } = active.region;
    let (start_stable, start_col, end_stable, end_col) =
        normalize_selection_rect(anchor_stable_row, anchor_col, focus_stable_row, focus_col);
    // Point selection (anchor == focus): nothing is selected yet.
    if start_stable == end_stable && start_col == end_col {
        return None;
    }
    debug_assert_eq!(
        base_stable, start_stable,
        "caller must fetch the normalized stable range and pass base_stable == start_stable"
    );
    let start_off = (start_stable - base_stable).max(0) as usize;
    let end_off = (end_stable - base_stable).max(0) as usize;
    Some(extract_text_from_grid(
        lines, cols, start_off, start_col, end_off, end_col, default_bg,
    ))
}

/// Normalize anchor/focus into top-left / bottom-right inclusive bounds, in
/// stable-row coordinates. Returns `(start_stable, start_col, end_stable, end_col)`.
fn normalize_selection_rect(
    anchor_stable_row: isize,
    anchor_col: usize,
    focus_stable_row: isize,
    focus_col: usize,
) -> (isize, usize, isize, usize) {
    let start_stable = anchor_stable_row.min(focus_stable_row);
    let start_col = if anchor_stable_row < focus_stable_row {
        anchor_col
    } else if focus_stable_row < anchor_stable_row {
        focus_col
    } else {
        anchor_col.min(focus_col)
    };
    let end_stable = anchor_stable_row.max(focus_stable_row);
    let end_col = if focus_stable_row > anchor_stable_row {
        focus_col
    } else if anchor_stable_row > focus_stable_row {
        anchor_col
    } else {
        anchor_col.max(focus_col)
    };
    (start_stable, start_col, end_stable, end_col)
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
