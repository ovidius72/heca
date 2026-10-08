use super::build_selection_overlay;
use crate::app::selection_model::{
    SelectionOwner, SelectionRegion, SelectionSource, SelectionState,
};
use crate::chrome::terminal::TerminalId;
use heca_config::theme::Color;
use heca_core::backend::TerminalSnapshot;

fn selection_snapshot(cols: usize, rows: usize, top_stable: isize) -> TerminalSnapshot {
    TerminalSnapshot {
        cols,
        rows,
        cell_w: 8.0,
        cell_h: 12.0,
        default_fg: [1.0; 4],
        default_bg: [0.0, 0.0, 0.0, 1.0],
        cursor_color: [1.0; 4],
        cursor: heca_core::backend::TerminalCursor {
            col: 0,
            row: 0,
            visible: true,
            shape: heca_core::backend::TerminalCursorShape::Block,
        },
        lines: Vec::new(),
        viewport_offset: 0,
        at_bottom: true,
        scrollback_rows: rows,
        viewport_top_stable_row: top_stable,
        hyperlinks: Vec::new(),
        graphics: Vec::new(),
        images: Vec::new(),
    }
}

#[test]
fn build_selection_overlay_returns_none_when_inactive() {
    let selection = SelectionState::new();
    let accent = Color {
        r: 100,
        g: 150,
        b: 200,
        a: 255,
    };
    let snap = selection_snapshot(20, 10, 0);
    assert!(build_selection_overlay(&selection, TerminalId(1), &snap, &accent).is_none());
}

#[test]
fn build_selection_overlay_returns_none_when_owner_mismatch() {
    let mut selection = SelectionState::new();
    selection.begin(
        SelectionOwner(TerminalId(7)),
        SelectionSource::MouseDrag,
        SelectionRegion::HostGrid {
            anchor_stable_row: 2,
            anchor_col: 3,
            focus_stable_row: 2,
            focus_col: 3,
        },
    );
    let accent = Color {
        r: 100,
        g: 150,
        b: 200,
        a: 255,
    };
    let snap = selection_snapshot(20, 10, 0);
    // Query with a different pane id -> None
    assert!(build_selection_overlay(&selection, TerminalId(1), &snap, &accent).is_none());
}

#[test]
fn build_selection_overlay_returns_none_for_backend_native() {
    let mut selection = SelectionState::new();
    selection.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::Rpc,
        SelectionRegion::BackendNative,
    );
    let accent = Color {
        r: 100,
        g: 150,
        b: 200,
        a: 255,
    };
    let snap = selection_snapshot(20, 10, 0);
    assert!(build_selection_overlay(&selection, TerminalId(1), &snap, &accent).is_none());
}

#[test]
fn build_selection_overlay_returns_overlay_for_host_grid_on_owning_pane() {
    let mut selection = SelectionState::new();
    selection.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion::HostGrid {
            anchor_stable_row: 2,
            anchor_col: 3,
            focus_stable_row: 2,
            focus_col: 3,
        },
    );
    selection.update_focus(5, 9);
    let accent = Color {
        r: 100,
        g: 150,
        b: 200,
        a: 255,
    };
    let snap = selection_snapshot(20, 10, 0);
    let overlay = build_selection_overlay(&selection, TerminalId(1), &snap, &accent);
    assert!(overlay.is_some());
    let overlay = overlay.unwrap();
    assert_eq!(overlay.spans.len(), 4);
    assert_eq!(overlay.spans[0].row, 2);
    assert_eq!(overlay.spans[0].start_col, 3);
    assert_eq!(overlay.spans[0].end_col, 19);
    assert_eq!(overlay.spans[3].row, 5);
    assert_eq!(overlay.spans[3].start_col, 0);
    assert_eq!(overlay.spans[3].end_col, 9);
    // Color should be accent / 255 with 0.25 alpha
    assert_eq!(
        overlay.color,
        [100.0 / 255.0, 150.0 / 255.0, 200.0 / 255.0, 0.25]
    );
}

#[test]
fn build_selection_overlay_handles_reverse_multiline_selection() {
    let mut selection = SelectionState::new();
    selection.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion::HostGrid {
            anchor_stable_row: 8,
            anchor_col: 12,
            focus_stable_row: 8,
            focus_col: 12,
        },
    );
    selection.update_focus(4, 3);
    let accent = Color {
        r: 100,
        g: 150,
        b: 200,
        a: 255,
    };
    let snap = selection_snapshot(20, 10, 0);
    let overlay = build_selection_overlay(&selection, TerminalId(1), &snap, &accent).unwrap();
    assert_eq!(overlay.spans.len(), 5);
    assert_eq!(overlay.spans[0].row, 4);
    assert_eq!(overlay.spans[0].start_col, 3);
    assert_eq!(overlay.spans[0].end_col, 19);
    assert_eq!(overlay.spans[4].row, 8);
    assert_eq!(overlay.spans[4].start_col, 0);
    assert_eq!(overlay.spans[4].end_col, 12);
}

#[test]
fn build_selection_overlay_caret_returns_caret_indicator() {
    let mut selection = SelectionState::new();
    selection.set_caret(SelectionOwner(TerminalId(1)), 3, 7);
    let accent = Color {
        r: 100,
        g: 150,
        b: 200,
        a: 255,
    };
    let snap = selection_snapshot(20, 10, 0);
    let overlay = build_selection_overlay(&selection, TerminalId(1), &snap, &accent).unwrap();
    // Caret-only state: no selection spans.
    assert!(overlay.spans.is_empty());
    // But we get a caret indicator at the caret position.
    let caret = overlay
        .caret
        .expect("caret should be present in caret-only state");
    assert_eq!((caret.row, caret.col), (3, 7));
    assert!(
        !caret.is_selection_endpoint,
        "caret-only should not be a selection endpoint"
    );
}

#[test]
fn build_selection_overlay_caret_owner_mismatch_returns_none() {
    let mut selection = SelectionState::new();
    selection.set_caret(SelectionOwner(TerminalId(2)), 0, 0);
    let accent = Color {
        r: 100,
        g: 150,
        b: 200,
        a: 255,
    };
    let snap = selection_snapshot(20, 10, 0);
    assert!(build_selection_overlay(&selection, TerminalId(1), &snap, &accent).is_none());
}

#[test]
fn build_selection_overlay_active_selection_has_focus_caret() {
    let mut selection = SelectionState::new();
    selection.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::KeyboardMode,
        SelectionRegion::HostGrid {
            anchor_stable_row: 2,
            anchor_col: 0,
            focus_stable_row: 4,
            focus_col: 5,
        },
    );
    let accent = Color {
        r: 100,
        g: 150,
        b: 200,
        a: 255,
    };
    let snap = selection_snapshot(20, 10, 0);
    let overlay = build_selection_overlay(&selection, TerminalId(1), &snap, &accent).unwrap();
    // Active selection: should have both selection spans and a focus-end caret.
    assert!(!overlay.spans.is_empty());
    let caret = overlay
        .caret
        .expect("focus caret should be present for active selection");
    assert_eq!((caret.row, caret.col), (4, 5));
    assert!(
        caret.is_selection_endpoint,
        "active selection caret should be a selection endpoint"
    );
}
