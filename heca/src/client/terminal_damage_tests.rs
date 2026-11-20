use super::changed_rows;
use heca_core::backend::{FakeBackend, HyperlinkSpan, PaneBackend, TerminalDamage, TerminalLine, TerminalRowRange, WireSnapshot};

fn screen(rows: &[&str]) -> WireSnapshot {
    let mut screen = FakeBackend::new(20, rows.len()).terminal_snapshot().unwrap().to_wire();
    let blank = screen.lines[0].cells[0].clone();
    screen.lines = rows.iter().map(|text| TerminalLine { cells: text.chars()
        .map(|ch| heca_core::backend::TerminalCell { text: ch.to_string(), ..blank.clone() }).collect() }).collect();
    screen
}

#[test]
fn only_the_rows_that_changed_are_redrawn() {
    let drawn = screen(&["a", "b", "c", "d"]);
    assert_eq!(changed_rows(&drawn, &drawn.clone()), TerminalDamage::None);
    let now = screen(&["a", "B", "C", "d"]);
    assert_eq!(changed_rows(&drawn, &now), TerminalDamage::Rows(vec![TerminalRowRange::new(1, 3)]));
    let now = screen(&["A", "b", "c", "D"]);
    assert_eq!(changed_rows(&drawn, &now),
        TerminalDamage::Rows(vec![TerminalRowRange::new(0, 1), TerminalRowRange::new(3, 4)]));
}

#[test]
fn the_cursor_alone_redraws_no_row() {
    let drawn = screen(&["a", "b"]);
    let mut now = drawn.clone();
    now.cursor.row = 1;
    now.cursor.col = 3;
    assert_eq!(changed_rows(&drawn, &now), TerminalDamage::None);
}

#[test]
fn a_link_that_comes_or_goes_redraws_its_row() {
    let drawn = screen(&["a", "b", "c"]);
    let mut now = drawn.clone();
    now.hyperlinks.push(HyperlinkSpan { row: 2, start_col: 0, end_col: 1, uri: "https://x".into() });
    assert_eq!(changed_rows(&drawn, &now), TerminalDamage::Rows(vec![TerminalRowRange::new(2, 3)]));
    assert_eq!(changed_rows(&now, &drawn), TerminalDamage::Rows(vec![TerminalRowRange::new(2, 3)]));
}

#[test]
fn what_moves_or_recolours_the_whole_grid_redraws_it_all() {
    let drawn = screen(&["a", "b"]);
    let mut scrolled = drawn.clone();
    scrolled.viewport_top_stable_row += 1;
    assert_eq!(changed_rows(&drawn, &scrolled), TerminalDamage::Full);
    let mut back = drawn.clone();
    back.viewport_offset = 1;
    assert_eq!(changed_rows(&drawn, &back), TerminalDamage::Full);
    let mut recoloured = drawn.clone();
    recoloured.default_bg = [0.5, 0.5, 0.5, 1.0];
    assert_eq!(changed_rows(&drawn, &recoloured), TerminalDamage::Full);
    assert_eq!(changed_rows(&drawn, &screen(&["a", "b", "c"])), TerminalDamage::Full);
}
