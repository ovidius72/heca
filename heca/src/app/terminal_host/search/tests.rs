use super::*;
use heca_core::backend::SearchMatch;

fn at(row: isize) -> SearchMatch {
    SearchMatch {
        stable_row: row,
        start_col: 0,
        end_col: 1,
    }
}

#[test]
fn no_matches_focus_nothing() {
    assert_eq!(nearest_at_or_above(&[], Some(10)), None);
    assert_eq!(nearest_at_or_above(&[], None), None);
}

/// The match the caret is on or the last one above it — so a new query starts where you are looking.
#[test]
fn the_focused_match_is_the_nearest_at_or_above_the_caret() {
    let found = [at(2), at(5), at(9)];
    assert_eq!(nearest_at_or_above(&found, Some(5)), Some(1), "on it");
    assert_eq!(nearest_at_or_above(&found, Some(8)), Some(1), "just above");
    assert_eq!(
        nearest_at_or_above(&found, Some(100)),
        Some(2),
        "below them all"
    );
}

/// Above every match there is nothing nearer, so it wraps to the last, like stepping back would.
#[test]
fn a_caret_above_every_match_wraps_to_the_last() {
    let found = [at(2), at(5), at(9)];
    assert_eq!(nearest_at_or_above(&found, Some(0)), Some(2));
}

/// With no caret at all it is the last match: the newest thing in the scrollback.
#[test]
fn no_caret_means_the_last_match() {
    let found = [at(2), at(5), at(9)];
    assert_eq!(nearest_at_or_above(&found, None), Some(2));
}
