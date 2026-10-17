use super::extract::extract_selection_text;
use super::model::*;
use crate::chrome::panes::terminal::TerminalId;
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 4,
        },
    );
    sel.end();
    let text = extract_selection_text(&sel, &lines, 0, 7, TEST_BG).unwrap();
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 1,
            focus_stable_row: 2,
            focus_col: 1,
        },
    );
    sel.end();
    // Row 0: start_col=1, end=3 → "bc" (trailing blank trimmed)
    // Row 1: full → "de"
    // Row 2: 0..=1 → "fg"
    let text = extract_selection_text(&sel, &lines, 0, 4, TEST_BG).unwrap();
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 1,
            anchor_col: 2,
            focus_stable_row: 0,
            focus_col: 0,
        },
    );
    sel.end();
    let text = extract_selection_text(&sel, &lines, 0, 3, TEST_BG).unwrap();
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 2,
        },
    );
    sel.end();
    // Should copy "中b" (filler cell at col 1 is skipped)
    let text = extract_selection_text(&sel, &lines, 0, 4, TEST_BG).unwrap();
    assert_eq!(text, "中b");
}

#[test]
fn extract_inactive_selection_returns_none() {
    let sel = SelectionState::new();
    let lines: Vec<TerminalLine> = vec![];
    assert!(extract_selection_text(&sel, &lines, 0, 0, TEST_BG).is_none());
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 3,
        },
    );
    sel.end();
    // Selecting entire row including trailing blanks — should get "ab"
    let text = extract_selection_text(&sel, &lines, 0, 4, TEST_BG).unwrap();
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 2,
        },
    );
    sel.end();
    // "a" + space + highlighted_space — both spaces must be preserved.
    let text = extract_selection_text(&sel, &lines, 0, 3, TEST_BG).unwrap();
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 3,
        },
    );
    sel.end();
    // Trailing blank_cell() has default_bg, so it's trimmed.
    let text = extract_selection_text(&sel, &lines, 0, 4, TEST_BG).unwrap();
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        // Start at column 1 (the filler of '中') — should include '中'
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 1,
            focus_stable_row: 0,
            focus_col: 2,
        },
    );
    sel.end();
    // Normalized: start=(0,1), end=(0,2)
    // The filler at col 1 walks back to col 0 (the anchor '中')
    // Expected: "\u{4e2d}b" (wide char included, filler skipped)
    let text = extract_selection_text(&sel, &lines, 0, 4, TEST_BG).unwrap();
    assert_eq!(text, "\u{4e2d}b");
}
