use super::model::*;
use crate::chrome::panes::terminal::TerminalId;

#[test]
fn new_selection_is_inactive() {
    let s = SelectionState::new();
    assert!(!s.is_active());
    assert!(s.owner().is_none());
    assert!(s.active().is_none());
    assert!(s.region().is_none());
}

#[test]
fn default_matches_new() {
    assert_eq!(SelectionState::default(), SelectionState::new());
}

#[test]
fn cursor_cell_reads_caret_and_focus_endpoints() {
    // Inactive → no cursor.
    let mut s = SelectionState::new();
    assert_eq!(s.cursor_cell(), None);

    // Caret-only state → the caret cell.
    s.set_caret(SelectionOwner(TerminalId(7)), 3, 5);
    assert_eq!(s.cursor_cell(), Some((SelectionOwner(TerminalId(7)), 3, 5)));

    // Active selection → the moving FOCUS endpoint, not the anchor.
    s.begin(
        SelectionOwner(TerminalId(9)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 2,
            anchor_col: 1,
            focus_stable_row: 8,
            focus_col: 4,
        },
    );
    assert_eq!(s.cursor_cell(), Some((SelectionOwner(TerminalId(9)), 8, 4)));
}

#[test]
fn begin_transitions_to_selecting() {
    let mut s = SelectionState::new();
    s.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 0,
        },
    );
    assert!(s.is_selecting());
    assert_eq!(s.owner(), Some(SelectionOwner(TerminalId(1))));
    assert_eq!(s.source(), Some(SelectionSource::MouseDrag));
    assert_eq!(
        s.region(),
        Some(&SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 0,
        })
    );
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 0,
        },
    );
    s.update_focus(0, 10);
    s.end();
    s.update_focus(0, 99);
    assert_eq!(
        s.region(),
        Some(&SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 10,
        })
    );
    assert!(s.has_selection());
}

#[test]
fn update_focus_changes_focus() {
    let mut s = SelectionState::new();
    s.begin(
        SelectionOwner(TerminalId(7)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 2,
            anchor_col: 3,
            focus_stable_row: 2,
            focus_col: 3,
        },
    );
    s.update_focus(4, 5);
    assert_eq!(
        s.region(),
        Some(&SelectionRegion {
            anchor_stable_row: 2,
            anchor_col: 3,
            focus_stable_row: 4,
            focus_col: 5,
        })
    );
}

#[test]
fn end_confirms_selection() {
    let mut s = SelectionState::new();
    s.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 0,
        },
    );
    s.update_focus(0, 5);
    s.end();
    assert!(s.has_selection());
    assert!(!s.is_selecting());
    assert_eq!(
        s.region(),
        Some(&SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 5,
        })
    );
}

#[test]
fn end_has_no_effect_when_not_selecting() {
    let mut s = SelectionState::new();
    s.end();
    assert!(!s.has_selection());
    assert!(!s.is_active());
}

#[test]
fn end_is_idempotent_when_selected() {
    let mut s = SelectionState::new();
    s.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 5,
        },
    );
    s.end();
    assert!(s.has_selection());
    s.end();
    assert!(s.has_selection());
    assert_eq!(
        s.region(),
        Some(&SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 5,
        })
    );
}

#[test]
fn clear_resets_state() {
    let mut s = SelectionState::new();
    s.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 0,
        },
    );
    s.end();
    s.clear();
    assert!(!s.is_active());
    assert!(s.owner().is_none());
    assert!(s.source().is_none());
    assert!(s.region().is_none());
}

#[test]
fn caret_state_is_pre_selection() {
    let mut s = SelectionState::new();
    s.set_caret(SelectionOwner(TerminalId(1)), 3, 5);
    // Caret is not "active" (no selection gesture), but it has an owner.
    assert!(!s.is_active());
    assert!(s.is_caret());
    assert_eq!(s.owner(), Some(SelectionOwner(TerminalId(1))));
    assert!(s.active().is_none()); // no selection payload
    assert_eq!(s.caret_pos(), Some((3, 5)));
}

#[test]
fn caret_move_updates_position() {
    let mut s = SelectionState::new();
    s.set_caret(SelectionOwner(TerminalId(1)), 0, 0);
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
    s.set_caret(SelectionOwner(TerminalId(1)), 3, 5);
    s.begin_selection_from_caret(SelectionSource::KeyboardMode);
    // Now in Selecting state with anchor and focus at the caret position.
    assert!(s.is_active());
    assert!(!s.is_caret());
    assert!(s.is_selecting());
    let active = s.active().unwrap();
    assert_eq!(active.owner, SelectionOwner(TerminalId(1)));
    assert_eq!(active.source, SelectionSource::KeyboardMode);
    let SelectionRegion {
        anchor_stable_row,
        anchor_col,
        focus_stable_row,
        focus_col,
    } = active.region;
    assert_eq!(
        (anchor_stable_row, anchor_col, focus_stable_row, focus_col),
        (3, 5, 3, 5)
    );
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
        SelectionOwner(TerminalId(1)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 2,
            focus_stable_row: 3,
            focus_col: 5,
        },
    );
    s.toggle_selection_endpoint();
    let SelectionRegion {
        anchor_stable_row,
        anchor_col,
        focus_stable_row,
        focus_col,
    } = s.active().unwrap().region;
    // After toggle, anchor and focus are swapped.
    assert_eq!(
        (anchor_stable_row, anchor_col, focus_stable_row, focus_col),
        (3, 5, 0, 2)
    );
}

#[test]
fn toggle_endpoint_no_op_on_caret() {
    let mut s = SelectionState::new();
    s.set_caret(SelectionOwner(TerminalId(1)), 0, 0);
    s.toggle_selection_endpoint();
    // Still in caret state.
    assert!(s.is_caret());
}

#[test]
fn toggle_endpoint_works_on_selected_state() {
    let mut s = SelectionState::new();
    s.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 1,
            anchor_col: 0,
            focus_stable_row: 5,
            focus_col: 3,
        },
    );
    s.end(); // Confirm → Selected state
    assert!(s.has_selection());
    s.toggle_selection_endpoint();
    let SelectionRegion {
        anchor_stable_row,
        anchor_col,
        focus_stable_row,
        focus_col,
    } = s.active().unwrap().region;
    assert_eq!(
        (anchor_stable_row, anchor_col, focus_stable_row, focus_col),
        (5, 3, 1, 0)
    );
}

#[test]
fn clear_resets_caret_to_inactive() {
    let mut s = SelectionState::new();
    s.set_caret(SelectionOwner(TerminalId(1)), 0, 0);
    s.clear();
    assert!(!s.is_caret());
    assert!(!s.is_active());
}

#[test]
fn begin_resets_prior_selection() {
    let mut s = SelectionState::new();
    s.begin(
        SelectionOwner(TerminalId(1)),
        SelectionSource::MouseDrag,
        SelectionRegion {
            anchor_stable_row: 0,
            anchor_col: 0,
            focus_stable_row: 0,
            focus_col: 0,
        },
    );
    s.update_focus(0, 10);
    s.end();
    s.begin(
        SelectionOwner(TerminalId(2)),
        SelectionSource::KeyboardMode,
        SelectionRegion {
            anchor_stable_row: 4,
            anchor_col: 1,
            focus_stable_row: 4,
            focus_col: 1,
        },
    );
    assert!(s.is_selecting());
    assert_eq!(s.owner(), Some(SelectionOwner(TerminalId(2))));
    assert_eq!(s.source(), Some(SelectionSource::KeyboardMode));
    assert_eq!(
        s.region(),
        Some(&SelectionRegion {
            anchor_stable_row: 4,
            anchor_col: 1,
            focus_stable_row: 4,
            focus_col: 1,
        })
    );
}
